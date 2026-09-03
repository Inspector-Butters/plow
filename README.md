# Plow

Plow turns active Codex threads into a small robot farm. Repositories become field plots, top-level threads become lead workers, and sub-agents work nearby. Approval requests, questions, failures, and completed turns are surfaced as calm but unmistakable attention states.

## Install a release

Download the latest build from [GitHub Releases](https://github.com/Inspector-Butters/plow/releases/latest), or use the installer:

```sh
curl -fsSLO https://raw.githubusercontent.com/Inspector-Butters/plow/main/scripts/install.sh
sh install.sh
```

If the repository is private, either download from the Releases page while signed into GitHub, or give a fine-grained token read access to the repository contents and run:

```sh
curl -fsSL \
  -H "Authorization: Bearer $GITHUB_TOKEN" \
  -H "Accept: application/vnd.github.raw+json" \
  https://api.github.com/repos/Inspector-Butters/plow/contents/scripts/install.sh \
  -o install.sh
sh install.sh
```

The shorter public installer command works directly if the repository is made public later.

On Linux, the installer puts the matching AppImage at `~/.local/bin/plow` (or `$XDG_BIN_HOME/plow`). On macOS, it downloads and opens the DMG for Apple Silicon or Intel; drag Plow into Applications. The GitHub macOS builds are ad-hoc signed rather than notarized, so the first launch may require right-clicking Plow and choosing **Open**, or allowing it in **System Settings → Privacy & Security**.

Plow requires a current [Codex CLI](https://learn.chatgpt.com/docs/developer-commands?surface=cli) with managed app-server daemon support. Sessions monitored by Plow should connect to the shared daemon:

```sh
codex --remote unix://
```

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
