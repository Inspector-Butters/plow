# Plow

Plow turns active Codex threads into a small robot farm. Repositories become field plots, top-level threads become lead workers, and sub-agents work nearby. Approval requests, questions, failures, and completed turns are surfaced as calm but unmistakable attention states.

## Install a release

Download the latest build from [GitHub Releases](https://github.com/Inspector-Butters/plow/releases/latest), or use the installer:

```sh
curl -fsSLO https://raw.githubusercontent.com/Inspector-Butters/plow/main/scripts/install.sh
sh install.sh
```

Plow checks the latest GitHub release after launch. When a newer signed build is available, it asks before downloading anything, shows installation progress, and relaunches into the new version. Version 0.3.0 is the first self-updating release, so earlier versions need one final manual install.

On Linux, the installer puts the matching AppImage at `~/.local/bin/plow` (or `$XDG_BIN_HOME/plow`). On macOS, it downloads and opens the DMG for Apple Silicon or Intel; drag Plow into Applications. The GitHub macOS builds are ad-hoc signed rather than notarized, so the first launch may require right-clicking Plow and choosing **Open**, or allowing it in **System Settings → Privacy & Security**.

Plow requires a current [Codex CLI](https://learn.chatgpt.com/docs/developer-commands?surface=cli) with managed app-server daemon support. Sessions monitored by Plow should connect to the shared daemon:

```sh
codex --remote unix://
```

To start sessions from Plow, set **Settings → Development home folder** to the absolute path containing your projects. The **Start agent** button lists its immediate subfolders and opens a new shared-daemon Codex session in the selected project.

Plow first looks for the daemon-capable standalone executable at `$CODEX_HOME/packages/standalone/current/codex` (or `~/.codex/packages/standalone/current/codex`), then checks `PATH` and common install locations. You can override detection from **Settings → Codex executable** with an absolute path; changing it reconnects the monitor without restarting Plow.

An npm or Homebrew installation may run the normal Codex CLI but still be unable to start the managed daemon. In that case, install the standalone build and leave Plow on automatic detection:

```sh
curl -fsSL https://chatgpt.com/codex/install.sh | sh
```

`CODEX_INSTALL_DIR` changes the user-facing command location, while the standalone package used by the daemon remains under `CODEX_HOME`.

On Linux, terminal handoff supports GNOME Terminal, Console (`kgx`), Ptyxis, Konsole, Xfce Terminal, MATE Terminal, Kitty, WezTerm, Foot, Alacritty, Tilix, LXTerminal, xterm, and the standard desktop terminal launchers. Plow tries another installed terminal when a launcher fails immediately.

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

### Release signing

Self-updates are verified with Tauri's mandatory updater signatures. The public key is embedded in the application; the private key must never be committed. Release maintainers must set the repository Actions secret `TAURI_SIGNING_PRIVATE_KEY` to the contents of the matching private key before pushing a version tag. The release workflow creates and publishes `latest.json` plus the signed Linux and macOS updater bundles.

With GitHub CLI authenticated for the repository, configure that secret without printing the key:

```sh
gh secret set TAURI_SIGNING_PRIVATE_KEY < /secure/path/to/updater.key
```

Back up that key securely. Replacing or losing it prevents installed copies from accepting future updates.
