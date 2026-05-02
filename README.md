# SpriteAnimte

SpriteAnimte is a Tauri 2 desktop app for AI image generation and sprite-sheet preview workflows. It combines a Vite/TypeScript frontend with a Rust backend for local file handling, image processing, workbench persistence, and Responses API image-generation requests.

## Features

- Generate images through a fixed `/responses` flow using the `image_generation` tool.
- Keep generated and local images in a persistent workbench.
- Preview sprite sheets with adjustable row/column grids and crop regions.
- Auto-detect per-frame content bounds for cleaner sprite slicing.
- Export selected animation frames as PNG files.

## Requirements

- Node.js 18 or newer
- npm
- Rust stable toolchain
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

The app stores personal runtime configuration under the user data directory, for example `~/.local/share/sprite-animte/`. A root `config.json` is treated as local migration/runtime data and is ignored by Git. Use `config.example.json` only as a safe reference.

## Common Commands

```bash
npm run typecheck        # TypeScript validation
npm run build            # frontend production build
npm run test:rust        # Rust unit tests
npm test                 # TypeScript check + Rust tests
npm run check:rust       # Rust cargo check
npm run tauri:build      # Tauri package build
./scripts/test.sh --all  # extended local validation
```

## Project Structure

```text
index.html              # Vite entry HTML
src/main.ts             # frontend bootstrap
src/pages/              # generator and sprite workflow controllers
src/widgets/            # reusable UI widgets
src/api/commands.ts     # Tauri invoke wrappers and DTO types
src-tauri/src/          # Rust app state, commands, API client, image processing
src-tauri/capabilities/ # Tauri permissions
scripts/                # local run, test, deploy, and packaging helpers
```

Generated folders such as `dist/`, `node_modules/`, `src-tauri/target/`, `logs/`, and `output/` should not be committed.

## Configuration And Secrets

Do not commit real API keys, proxy credentials, generated images, logs, or local config files. Keep `config.json` local. If documenting a setup, use placeholders like `https://your-relay.example/v1`.

## GitHub Release

The `Release` workflow builds Linux, Windows, and macOS bundles. Run it from GitHub Actions with `Run workflow`. By default it publishes a draft release using the version in `src-tauri/tauri.conf.json`, such as `v0.1.0`.

- Leave `ref` empty to build the selected branch or tag.
- Set `ref` to a branch, tag, or commit SHA when rebuilding a specific revision.
- Leave `release_tag` empty to use the Tauri config version, or set it explicitly, for example `v0.2.0`.
- Keep `draft` enabled until the generated assets have been checked.

## License

No license has been selected yet. Add a `LICENSE` file before publishing if this repository should be open source.
