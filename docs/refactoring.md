# Architecture Refactoring Record

This document is the source of truth for repository-wide structural work. Update it after each completed extraction so responsibilities are not duplicated or lost.

## Invariants

- Preserve the image generation, video generation, sprite editing, video frame extraction, grouped AI redraw, workbench, asset library, configuration, and FFmpeg workflows.
- Keep every authored code or UI file at 500 lines or fewer. Generated output and dependency directories are excluded.
- Keep Tauri invocation names and serialized field names stable unless the corresponding frontend and backend are changed together.
- Keep API keys in local configuration only; grouped-redraw manifests must never persist credentials.
- Keep resumable UI state in the versioned workspace snapshot; persist binary edits as stable application assets, never Blob URLs or startup-cleaned temporary paths.
- Fail explicitly at system boundaries. Do not add silent fallback, duplicate validation, or compatibility branches.
- Preserve user-owned workspace changes and avoid repository-wide formatting.

## Target Boundaries

Frontend code is grouped by feature (`generator`, `video-sprite`, `sprite`) with small page coordinators. Shared Tauri access is split by domain under `src/api/`. HTML fragments and CSS files follow the same feature boundaries. Backend commands remain thin and delegate to domain modules. API transport, provider protocols, image processing, configuration persistence, and FFmpeg operations are separate responsibilities.

## Migration Checklist

- [x] Implement and verify grouped AI redraw, including partial batches and resumable runs.
- [x] Record architecture invariants, target boundaries, and validation commands.
- [x] Add `npm run check:lines` to report authored files over 500 lines.
- [x] Split grouped-redraw backend storage, planning, and image pipeline.
- [x] Split frontend Tauri APIs by domain.
- [x] Split video-sprite, sprite editor, and generator coordinators.
- [x] Split `index.html` and `src/style.css` by feature.
- [x] Split sprite/video commands, generation commands, API providers, configuration, image processing, and FFmpeg tools.
- [x] Enable the line limit in the default validation chain after legacy violations reach zero.
- [x] Move image, sprite, and video modules under explicit `src/features/` business boundaries.
- [x] Enable TypeScript unused-symbol checks and remove production Rust wildcard imports.
- [x] Remove frontend runtime import cycles and concrete cross-page controller dependencies.
- [x] Centralize path-component sanitizing, prompt-optimizer API validation, media-picker construction, request validation, URL scanning, and HTTP send-error mapping.
- [x] Split FFmpeg platform manifests and download/install logic from the Tauri command façade.
- [x] Move workbench tests out of the production store module.
- [x] Audit duplicate blocks, dependency direction, strict Clippy warnings, and the 500-line invariant.
- [x] Remove obsolete modules, compatibility migrations, duplicate commands/events, silent fallbacks, and replaced implementations.
- [x] Add durable schema-v3 recovery for one active task and its current stage.
- [x] Replace the technical page tabs with a task launcher, one staged workspace, and a settings center.
- [x] Extract API profiles and tool configuration from the image controller into `src/settings/`.
- [x] Add the pure batch `sprite-anime-cli`, shared data locks, standalone archives, and packaged Tauri sidecar.

## Completed Structure

- Page classes own one state instance; domain method sets are registered directly on their prototypes, so cross-domain coordination does not require duplicate state or forwarding methods.
- `src/features/image/`, `src/features/sprite/`, and `src/features/video/` are business roots. Sprite and video method sets are isolated under `controller/`; algorithms, models, workers, and renderers remain at the feature root.
- `src/settings/` exclusively owns API profiles, API modes, configuration import/export, checks, and FFmpeg paths. Image and video features consume the settings provider; Sprite receives explicit sources from the task coordinator and imports no other feature controller.
- `src/workflows/` owns the finite task/stage model and the single primary action. Feature surfaces expose editing capabilities but no duplicate page navigation or hidden primary buttons.
- `src/api/` contains domain-specific Tauri contracts with `commands.ts` as a compatibility-free export surface.
- `src/ui/` and `src/styles/` mirror feature boundaries and are mounted/imported in deterministic order.
- Rust generation, sprite/video, redraw, tools, configuration, image processing, and API provider code use directory modules with command façades and focused internal services. `commands/tools/manifest.rs` owns platform metadata; `download.rs` owns network, extraction, and installation.
- `src-tauri/src/path_safety.rs` is the single filename-component sanitizer. Prompt-optimizer configuration validation is owned by `commands/generate/config_commands.rs`.

## Redundancy Cleanup

- Removed unused UI mockups, legacy page modules, the portable-data migration script, old monolithic Rust modules, superseded FFmpeg extraction code, and normal-flow debug logging.
- Removed duplicate prompt-history reads and the `get_prompt_history` command; `add_prompt_history` now returns the updated list from the single persistence operation.
- Removed duplicate command error events. Progress channels only report progress; command `Result` values are the sole failure path.
- Removed the `ApiHttpClient` pass-through wrapper, duplicated path sanitizers, temporary-directory path inference, no-op async `invoke` wrappers, and unused query/sprite extraction helpers.
- Removed redundant redraw manifest fields and warning-success cleanup behavior. Manifest cleanup and video log writes now fail explicitly.
- Removed boundary editor `strict`/old-value fallbacks and automatic-boundary default reconstruction. Required grid rectangles and signatures now cross the worker boundary explicitly, and invalid cell dimensions reach validation instead of being clamped first.
- Consolidated video-worker foreground bounds and transparency-mask detection into one pixel scan.
- Removed the unreachable post-AppDir loop check and arbitrary executable-name fallback from AppImage post-processing; the current `sprite-anime` binary contract now fails explicitly when violated.
- Made `config` and `image_processor` crate-private, which exposed and removed the unused active-profile lookup, transparent-copy wrapper, source-parent helper, and transparent-options default implementation.
- Restricted ratio parsing to the presets the UI can produce. Ratio DTOs now serialize only the frontend key; backend-only dimensions stay internal and are covered by a serialization test.
- Enforced one workbench selection invariant across generator and sprite workflows. Removed last-record selection fallbacks, forwarding methods, write-only class state, redundant thumbnail state, and duplicate canvas cleanup paths.
- Removed response and worker fields with no consumer, including duplicated frame dimensions, projected cell sizes, matting seed/color details, frame row/column coordinates, and redraw-plan values derivable from batches and grid dimensions.
- Removed unreachable matting branches after the seed and self-match invariants, plus fabricated `1x1`/full-image regions when sprite state is incomplete.
- Removed 268 early CSS declarations that were always overridden, then merged shared sprite/video workbench rules. The final selector scan reports no duplicate rule groups and no unused classes beyond data-URL tokens.
- Removed deployment placeholder-icon generation, unknown-mode fallbacks, optional-success packaging paths, hard-coded build metadata, stale lock cleanup, and permissive extra arguments. Scripts now require their actual inputs and fail when required artifacts are absent.
- Removed unused default parameters from sprite editing contracts, the zero-reference generator state getter, and an empty redraw `finally` block. Updated README contracts that still described the deleted migration tool and API-value reuse.
- Added an explicit prompt-optimizer call mode shared by configuration, API checks, and optimization requests. Removed domain/model-name protocol guessing; the field is now required by the strict current configuration schema.
- Added explicit GGGB media protocols for `/images/generations`, `/images/edits` JSON/multipart, `/videos`, `/videos/generations`, `/videos/edits`, and `/videos/extensions`. Standard video creation now uses the documented JSON contract with integer seconds and shares one direct-response, polling, and authenticated-download path.
- Added the versioned workspace snapshot under `SpriteAnimteData/workspace.json`. Schema v3 saves exactly one active task, its finite stage, generation inputs, video extraction/redraw state, sprite grid/boundary/playback state, and dirty matting pixels. Schema v2 is rejected explicitly and reset through the recovery dialog instead of being migrated.
- Workspace writes use a temporary file commit, validate every persisted `*Path` against the application data root, reject unknown schema versions, and surface corrupt snapshots without overwriting them. Window close requests commit both `config.json` and the final workspace snapshot before destroying the Tauri window.
- Restored redraw runs now expose one unambiguous action path: retry the bound API snapshot, or delete the run before changing its model/protocol. Removed the redundant replace-existing request flag, disabled new-run creation while a run exists, translated restored states, and removed the sticky action panel that obscured batch errors.
- Corrected the GGGB asynchronous video contract from authenticated live traffic: creation returns `request_id` without status, polling returns status without repeating the ID, and completion returns the media at `video.url`. Status GETs retry at most three times after observed connection resets; creation POSTs are never retried because that could duplicate billing.
- Aligned empty proxy configuration with the settings contract: API clients now use reqwest's environment proxy discovery instead of disabling all proxies with `no_proxy()`. Explicit profile proxies still override the environment, and media download failures name both configuration paths.
- Centralized generation-constraint rendering, connected-region erase, FFmpeg extraction progress, and sprite-bound detection in shared Rust sources used by both desktop commands and CLI. Generation forms persist only structured choices and user-authored prompts; exact layout, identity, framing, background, fixed-camera, and loop instructions are appended once at request time. Removed the replaced TypeScript worker/algorithm implementations and their stale Node tests.
- Added `sprite-anime-cli` with schema-versioned JSON output, stderr JSONL progress, explicit exit codes, profile/env secret resolution, non-blocking cross-process locks, strict config export rules, resumable redraw commands, and complete image/video/sprite/local-data command groups. Release builds package it both as a Tauri sidecar and a standalone archive.
- Removed the completed workspace v1-to-v2 migration after the live workspace was upgraded, along with test-only production defaults, the redraw-only constant reference flag, and duplicate constraint-control synchronization during restore.
- Removed the remaining prompt-optimizer field migration and made both configuration levels reject unknown fields. AppImage post-processing now fails when required output is absent and repacks only the AppImage paired with its exact AppDir instead of guessing another artifact.
- Removed the empty CLI Cargo feature and its stale build flags, plus the orphaned pixel-test helper and unused sprite color DTO left after moving image algorithms to Rust.
- Centralized prompt-history and workbench persistence, validation, and cross-process locking in shared services used by desktop commands and CLI generation/local-data paths.
- Configuration export now normalizes and writes the supplied value without replacing the active configuration; JSON extension handling has one owner. Imported configuration returns the exact normalized value persisted to disk.
- FFmpeg extraction progress carries media time as structured data instead of parsing localized message text. Sprite-layout reductions now reject empty invalid grids and derive bounds from proven elements instead of fabricating zero/one-size values.
- Removed the ignored explicit file unlock and relies on the `fs2`-verified file-handle lifetime contract. CLI output serialization failures now produce an internal error and nonzero exit instead of a fake human-readable success value.
- Removed the nested unreachable video-mode branch; each validated standard video mode maps directly to its endpoint.
- Added ordered two-reference image requests for grouped redraw. The first reference is always the current target grid; later batches add the previous generated final frame as the second continuity anchor. Rust now owns the batch prompt and execution references for both desktop and CLI, enforces predecessor success before generation, and rejects text-only `/images/generations` before creating a run without adding fallback composition or manifest fields.
- Completed the ordered-reference cleanup: removed Responses/Chat pass-through wrappers, reused one `ImageApiRequest` across all image protocols, borrowed base64 reference data instead of cloning it, centralized edit-reference and redraw-batch status validation, and stopped rebuilding the full frontend redraw plan for every batch.
- Replaced all three media/optimizer mode string dispatches with validated finite enums, eliminating unreachable invalid-mode branches after configuration parsing. Unknown image resolutions now fail before any API request instead of being silently treated as 1K, and the redundant redraw `run_id` recheck plus the `div_ceil` forwarding helper were removed.
- Split multipart request-capture tests out of the image API test module to retain the 500-line invariant. Reduced internal Rust and TypeScript visibility for helpers and nested DTOs with no external consumer; module reachability checks found no orphaned production source files.

Reviewed and retained: editable numeric-input normalization, unloaded-video zero dimensions, provider-specific media response parsing, AppImage WebKit path selection, and process-cleanup best-effort commands. These represent active UI states, protocol contracts, or operational cleanup rather than obsolete compatibility paths.

## Audit Results

- Authored code and UI files over 500 lines: **0**.
- Production TypeScript runtime import cycles: **0**. Method modules only type-import their owning page class.
- Cross-file duplicate 12-line production windows: **0**. Remaining similar blocks represent distinct provider protocols rather than copied rules.
- Production Rust `use super::*` imports: **0**; wildcard imports remain confined to test modules.
- TypeScript `noUnusedLocals` and `noUnusedParameters` are enabled. Rust passes Clippy for all targets with warnings denied.
- Missing settings configuration now fails explicitly; no feature imports another feature's concrete controller.
- Production TypeScript exports have at least one active consumer; compiler and Clippy unused-symbol checks report no dead declarations.
- All 79 production TypeScript runtime modules are reachable from the app or worker entries. All 92 Rust source files are present in the crate module tree. Shell and Node scripts contain no zero-call function declarations, and production sources contain no cross-file duplicate 12-line windows.

## Baseline Violations

At the start of this refactor, 14 files exceeded the limit: `generator.ts`, `api_client.rs`, `style.css`, `video-sprite.ts`, `commands/sprite.rs`, `commands/generate.rs`, `pages/sprite.ts`, `image_processor.rs`, `commands/redraw.rs`, `config.rs`, `index.html`, `api/commands.ts`, `commands/tools.rs`, and `migrate-portable-data.mjs`. All baseline violations are resolved; `npm run check:lines` is the permanent gate.

## Validation

Use focused tests after each move, then run `npm run typecheck`, `npm run check:rust`, and `git diff --check`. Before completion run `npm test`, `npm run build`, `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`, `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`, and a real Chromium smoke test of all three page transitions.

Latest full validation on 2026-07-19:

- `npm test`: passed line-limit, TypeScript, matting, workflow, grouped-redraw, unified-workspace, and 191 Rust tests.
- `npm run build`: produced the application and worker bundle from 96 transformed modules.
- `./scripts/test.sh --all`: passed Rust tests, TypeScript, production build, release check, and Clippy. Standalone all-target/all-feature Clippy also passed with warnings denied.
- Rust workspace tests cover schema-v2 rejection, empty schema-v3 workspaces, unknown fields, cross-task stages, path boundaries, atomic writes, temporary-snapshot cleanup, idempotent reset, and stable-asset preservation.
- Headless Chromium exercised the real Vite UI at 1440×900 and 390×844 with Tauri IPC mocked only at the system boundary. The task launcher, settings focus/close flow, image/video/Sprite task routing, video extension fields, action bar, and stage rail had no runtime errors or horizontal overflow.
- Chromium completed the image task from a local workbench image through optional matting, grid, bounds, 12-frame splitting, and preview/export. This exposed and verified the fix for Sprite region initialization ordering and the bounds-to-preview transition.
- The isolated Tauri development app started successfully with the strict current configuration and no workspace snapshot. The production TypeScript reachability scan found no orphaned runtime modules.
