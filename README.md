# SpriteAnimte

SpriteAnimte is a Tauri 2 desktop app for AI image generation, AI video generation, video-to-frame extraction, sprite-sheet editing, and animation preview workflows. It uses a Vite/TypeScript frontend and a Rust backend for local files, API calls, image processing, ffmpeg integration, persistence, and packaging.

## Features

- Manage multiple API profiles and switch them from the top toolbar.
- Generate images through `/responses`, `/chat/completions`, `/images/generations`, or `/images/edits` JSON/multipart.
- Generate, edit, or extend videos through `/chat/completions`, `/videos`, `/videos/generations`, `/videos/edits`, or `/videos/extensions`, then send the result into the video-to-sprite workflow.
- Import local videos, preview source ranges, crop the frame area, extract frames with ffmpeg, and save sprite sheets.
- Use ffmpeg fallback preview frames when the WebView cannot play a source video directly.
- Tune fallback preview extraction with `Preview FPS` and `Preview Limit`; tune playback speed with `Playback FPS`.
- Keep generated images, imported media, matted images, generated videos, frame exports, and GIF exports in a structured local asset library.
- Split sprite sheets, auto-detect content bounds, preview animation frames, and export PNG/GIF results.

## Requirements

- Node.js 18 or newer
- npm
- Rust stable toolchain
- ffmpeg and ffprobe, either available on `PATH` or configured in the app settings
- Linux Tauri build dependencies when packaging on Linux, such as GTK/WebKit libraries

## Getting Started

```bash
npm ci
npm run tauri:dev
```

For frontend-only development:

```bash
npm run dev
```

Runtime data is stored beside the executable or app bundle in `SpriteAnimteData/`. During development, this is usually under the Tauri target executable directory. The folder contains:

```text
SpriteAnimteData/
  config.json
  workbench_records.json
  logs/
  assets/
    generated-images/
    imported-images/
    imported-videos/
    generated-videos/
    matted-images/
    sprite-sheets/
    exported-frames/
    exported-gifs/
    temp/
```

Use `config.example.json` only as a placeholder reference. Real API keys, proxy URLs, generated assets, logs, and local `SpriteAnimteData/` folders must not be committed.

## Common Commands

```bash
npm run typecheck        # TypeScript validation
npm run build            # frontend production build
npm run test:matting     # matting algorithm checks
npm run check:cli        # CLI compilation check
npm run test:workflow    # workflow state checks
npm run test:rust        # Rust unit tests
npm test                 # TypeScript + Node tests + Rust tests
npm run check:rust       # Rust cargo check
npm run tauri:build      # Tauri package build
npm run cli -- --help    # run the batch CLI in development
npm run cli:build        # build the release CLI sidecar
./scripts/test.sh --all  # extended local validation, including Clippy
```

The shell wrappers in `scripts/` share helpers from `scripts/lib/common.sh`. `scripts/run.sh` starts development with hot reload, and `scripts/build.sh --release` creates release packages under `output/`.

## Command-Line Interface

The `sprite-anime-cli` binary exposes the desktop application's configuration, API checks, prompt optimization, image/video generation, matting and erase operations, FFmpeg probing/extraction, sprite detection/splitting/export, asset/workbench management, and resumable grouped redraw workflow without starting a WebView. It uses the same `SpriteAnimteData` directory by default and supports `--data-dir` or `SPRITE_ANIME_DATA_DIR` for explicit isolation.

API keys are read from the selected profile or `SPRITE_ANIME_IMAGE_API_KEY`, `SPRITE_ANIME_VIDEO_API_KEY`, and `SPRITE_ANIME_OPTIMIZER_API_KEY`; keys are never accepted as command arguments. See [docs/cli.md](docs/cli.md) for the complete command contract, JSON schemas, examples, and exit codes.

## Project Structure

```text
index.html              # minimal Vite entry document
src/main.ts             # frontend bootstrap and workspace wiring
src/api/                # domain-specific Tauri invoke wrappers and DTO types
src/features/           # image, video, and sprite task domains
src/settings/           # application settings and API profile ownership
src/workflows/          # unified task routing, presentation, and permissions
src/workspace/          # schema-v3 autosave and restore session
src/ui/                 # application shell and feature HTML fragments
src/styles/             # shell, feature, and responsive styles
src/utils/              # shared frontend helpers
src/widgets/            # reusable UI widgets
src-tauri/src/          # Rust app state, commands, API client, asset library, image processing
src-tauri/capabilities/ # Tauri permissions
scripts/                # local run, test, and packaging helpers
```

Generated folders such as `dist/`, `node_modules/`, `src-tauri/target/`, `src-tauri/gen/`, `SpriteAnimteData/`, `logs/`, and `output/` should not be committed.

## Configuration

Configuration is edited in the in-app settings dialog and saved to `SpriteAnimteData/config.json`. The app supports:

- multiple API profiles;
- image API key/base/proxy/model/call mode;
- video API key/base/proxy/model/call mode;
- prompt optimizer API settings;
- ffmpeg and ffprobe paths;
- config import and export.

Image call modes are `responses`, `chat_completions`, `images_generations`, `images_edits_json`, and `images_edits_multipart`. The two edit modes require a reference image. Video call modes are `chat_completions`, `videos`, `videos_generations`, `videos_edits`, and `videos_extensions`; edits and extensions require the original `video_id`. Standard video modes send JSON, poll `/videos/{video_id}`, and download `/videos/{video_id}/content?variant=video` with the same key. Image, video, and prompt-optimizer services each use their explicitly configured credentials, endpoints, models, and call modes.

GGGB video creation may return only `request_id`. Status responses may omit the ID and return the finished media as `video.url`; the client keeps the creation ID, polls with it, and downloads that URL. The documented content endpoint is used only when a completed response has no media URL. `grok-imagine-video-1.5-preview` may reject text-only generation; add a reference image or use `grok-imagine-video` for text-to-video.

For GGGB Gateway, use `https://api.imggb.top/v1` as both media API bases. Start with `images_generations` plus `grok-imagine-image` for text-to-image, and `videos` plus `grok-imagine-video` for video. Switch to an `/images/edits` mode when a reference image is present. `config.example.json` contains this secret-free profile.

Proxy fields override the process environment when filled. When left empty, reqwest reads `HTTP_PROXY`, `HTTPS_PROXY`, and `ALL_PROXY` (including lowercase variants). This also applies when downloading generated media from a different host such as `vidgen.x.ai`; packaged desktop launches that do not inherit these variables should use an explicit profile proxy.

## Security

Do not commit real API keys, proxy credentials, generated media, local config files, or logs. If documenting a setup, use placeholders such as `https://your-image-api.example/v1` and `https://your-video-api.example/v1`.

## GitHub Release

The `Release` workflow builds Linux, Windows, and macOS desktop bundles plus standalone `sprite-anime-cli` archives. The same CLI binary is embedded in desktop packages as a Tauri sidecar. Run the workflow from GitHub Actions with `Run workflow`, or push a tag matching `v*`. By default it publishes a draft release using the version in `src-tauri/tauri.conf.json`, such as `v0.1.0`.

- Leave `ref` empty to build the selected branch or tag.
- Set `ref` to a branch, tag, or commit SHA when rebuilding a specific revision.
- Leave `release_tag` empty to use the Tauri config version, or set it explicitly, for example `v0.2.0`.
- Keep `draft` enabled until the generated assets have been checked.

## License

No license has been selected yet. Add a `LICENSE` file before publishing if this repository should be open source.
