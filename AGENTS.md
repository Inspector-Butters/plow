# Plow contributor guide

Plow is a macOS and Linux desktop companion for monitoring Codex agents as robot workers on a farm. It is a Tauri 2 application with a React/TypeScript frontend and a Rust backend.

## Repository layout

- `src/`: React UI, farm scene, normalized worker state, and Tauri bridge. `FarmCanvas.tsx` owns the static/ambient field layers; `RobotWorker.tsx` owns task-specific character props and motion.
- `public/assets/`: project-owned raster artwork used by the farm.
- `src-tauri/src/`: Codex app-server supervisor, Unix-socket WebSocket protocol adapter, persistence, tray, and terminal handoff.
- `src-tauri/capabilities/`: Tauri permission declarations.

Keep Codex JSON-RPC details in Rust. The frontend consumes only normalized `Worker`, `ConnectionInfo`, and settings types from `src/types.ts`. Preserve the `HostTransport` boundary when adding remote hosts.

## Commands

- `npm install`: install frontend and Tauri CLI dependencies.
- `npm run dev`: run the browser UI with deterministic demo workers.
- `npm test`: run frontend unit tests once.
- `npm run check`: TypeScript-check the frontend.
- `npm run build`: check and build frontend assets.
- `npm run tauri dev`: compile and launch the native application.
- `cargo test --manifest-path src-tauri/Cargo.toml`: test the Rust backend.
- `cargo fmt --manifest-path src-tauri/Cargo.toml --check`: check Rust formatting.

Native development requires the Rust stable toolchain and the platform prerequisites documented by Tauri. Real monitoring requires a Codex CLI that supports the managed `app-server daemon`.

## Engineering rules

- Never read or copy Codex credentials, `auth.json`, session databases, or rollout files. Use only the documented app-server protocol.
- Never modify Codex databases. Plow metadata belongs under the Tauri application data directory.
- Treat protocol additions as optional. Ignore unknown JSON fields and show a clear health state when required methods are unavailable.
- Never pass an unvalidated thread ID to a process. Process arguments must be passed as an argv array, never interpolated into a shell command.
- Do not change a user's shell configuration. Onboarding may display and copy daemon launch commands only.
- Keep background monitoring quiet: deduplicate attention notifications by event key and notify only while the main window is unfocused.
- Every status must have an icon or text label in addition to color. All inspector actions must remain keyboard accessible.
- Respect `prefers-reduced-motion`; new animation must have a non-moving equivalent.
- Keep worker motion semantic: each activity needs a visibly distinct tool and action rather than a generic idle loop. Animate transforms and opacity, not layout properties.
- Keep the farm readable at 100 workers. Avoid per-frame React state updates and expensive per-worker canvas filters.
- Generated bitmap assets are project files. Record the prompt and source in `public/assets/README.md`; do not overwrite them without an explicit replacement request.

## Verification expectations

For frontend-only changes, run `npm test`, `npm run check`, and `npm run build`. For Rust or IPC changes, also run `cargo test` and `cargo fmt --check`. If the local machine lacks Rust or native system packages, state that limitation explicitly and still run the frontend checks.
