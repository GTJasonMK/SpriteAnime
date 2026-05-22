# SpriteAnimte

SpriteAnimte is a Tauri 2 desktop app for AI image generation, AI video generation, video-to-frame extraction, sprite-sheet editing, and animation preview workflows. It uses a Vite/TypeScript frontend and a Rust backend for local files, API calls, image processing, ffmpeg integration, persistence, and packaging.

## Features

- Manage multiple API profiles and switch them from the top toolbar.
- Generate images through either `/responses` or `/chat/completions`, depending on the selected profile.
- Generate videos through `/chat/completions` or `/videos`, then send them directly into the video-to-sprite workflow.
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
npm run test:bounds      # bounds detection checks
npm run test:workflow    # workflow state checks
npm run test:rust        # Rust unit tests
npm test                 # TypeScript + Node tests + Rust tests
npm run check:rust       # Rust cargo check
npm run tauri:build      # Tauri package build
./scripts/test.sh --all  # extended local validation, including Clippy
```

The shell wrappers in `scripts/` share helpers from `scripts/lib/common.sh`. `scripts/run.sh` starts development with hot reload, and `scripts/build.sh --release` creates release packages under `output/`.

## Project Structure

```text
index.html              # Vite entry HTML and page layout
src/main.ts             # frontend bootstrap and page wiring
src/api/commands.ts     # Tauri invoke wrappers and DTO types
src/pages/              # generator, video sprite, and sprite workflow controllers
src/pages/sprite/       # sprite-page subcontrollers and utilities
src/utils/              # shared frontend helpers
src/widgets/            # reusable UI widgets
src-tauri/src/          # Rust app state, commands, API client, asset library, image processing
src-tauri/capabilities/ # Tauri permissions
scripts/                # local run, test, migration, and packaging helpers
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

Image generation call modes are `responses` and `chat_completions`. Video generation call modes are `chat_completions` and `videos`. Empty video API key/base/proxy fields reuse the image API profile values.

## Security

Do not commit real API keys, proxy credentials, generated media, local config files, or logs. If documenting a setup, use placeholders such as `https://your-image-api.example/v1` and `https://your-video-api.example/v1`.

## GitHub Release

The `Release` workflow builds Linux, Windows, and macOS bundles. Run it from GitHub Actions with `Run workflow`, or push a tag matching `v*`. By default it publishes a draft release using the version in `src-tauri/tauri.conf.json`, such as `v0.1.0`.

- Leave `ref` empty to build the selected branch or tag.
- Set `ref` to a branch, tag, or commit SHA when rebuilding a specific revision.
- Leave `release_tag` empty to use the Tauri config version, or set it explicitly, for example `v0.2.0`.
- Keep `draft` enabled until the generated assets have been checked.

## License

No license has been selected yet. Add a `LICENSE` file before publishing if this repository should be open source.
