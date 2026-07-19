# Contributing

Thanks for taking the time to improve SpriteAnimte. This project is a Tauri 2 desktop app with a Vite/TypeScript frontend and a Rust backend.

## Development Setup

Install dependencies and start the app:

```bash
npm ci
npm run tauri:dev
```

For frontend-only work:

```bash
npm run dev
```

Install `ffmpeg` and `ffprobe`, or configure their paths in the app settings when testing video workflows.

## Validation

Run the focused checks before opening a pull request:

```bash
npm run typecheck
npm run build
npm run test:matting
npm run check:cli
npm run test:workflow
npm run test:rust
```

For broader local validation:

```bash
./scripts/test.sh --all
```

## Code Style

TypeScript uses strict mode, ES modules, two-space indentation, double quotes, and semicolons. Use kebab-case filenames, `PascalCase` classes, and `camelCase` functions and variables.

Rust follows `rustfmt` conventions. Tauri commands should keep the existing `Result<T, String>` style and return user-readable error messages.

## Data And Secrets

Do not commit real API keys, proxy credentials, generated media, logs, or local runtime data. Runtime data belongs in `SpriteAnimteData/`, which is ignored by Git. Use `config.example.json` for placeholders.

## Pull Requests

Pull requests should include:

- a concise summary of the user-visible change;
- validation commands that were run;
- notes about config, migration, packaging, or runtime data impacts;
- screenshots or recordings for UI changes;
- linked issues when applicable.

Keep changes focused. Avoid unrelated formatting churn or generated output.
