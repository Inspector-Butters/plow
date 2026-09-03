# Plow

Plow turns active Codex threads into a small robot farm. Repositories become field plots, top-level threads become lead workers, and sub-agents work nearby. Approval requests, questions, failures, and completed turns are surfaced as calm but unmistakable attention states.

## Development

```sh
npm install
npm run dev
```

Browser development uses deterministic demo workers. To run the native monitor, install the Rust stable toolchain and Tauri's platform prerequisites, then run:

```sh
npm run tauri dev
```

Plow starts the managed Codex app-server daemon and connects to its Unix-socket WebSocket control endpoint. Codex sessions should use the shared daemon:

```sh
codex --remote unix://
```

V1 is monitor-first. Attention buttons resume the selected thread in a terminal; approvals and conversation continue in Codex itself.

See [AGENTS.md](./AGENTS.md) for architecture, safety constraints, and verification commands.
