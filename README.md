# Plow

Plow turns active Codex threads into a small robot farm. Repositories become field plots, top-level threads become lead workers, and sub-agents work nearby. Approval requests, questions, failures, and completed turns are surfaced as calm but unmistakable attention states.

Use **Field** view for the animated farm, or switch to **Classic** for a text-only agent dashboard with direct terminal, copy, review, and detail controls. The selected view is remembered across launches.

Click **need attention** to see every agent waiting for approval or input, plus completed and failed work, with direct terminal and copy-command actions. Clicking the field closes agent details and other open overlays. Agent details show the configured model and reasoning effort, live context-window usage, and the bottom-left status bar shows account rate limits with stronger warnings as a limit runs low. These values appear when the connected Codex app-server and account expose them; rate limits require ChatGPT-backed authentication.

## Install a release

Download the latest build from [GitHub Releases](https://github.com/Inspector-Butters/plow/releases/latest), or use the installer:

```sh
curl -fsSLO https://raw.githubusercontent.com/Inspector-Butters/plow/main/scripts/install.sh
sh install.sh
```

Plow checks the latest GitHub release after launch. When a newer signed build is available, it asks before downloading anything, shows installation progress, and relaunches into the new version. Version 0.3.0 is the first self-updating release, so earlier versions need one final manual install.

On Linux, the installer puts the matching AppImage at `~/.local/bin/plow` (or `$XDG_BIN_HOME/plow`). On macOS, it downloads and opens the DMG for Apple Silicon or Intel; drag Plow into Applications. The GitHub macOS builds are ad-hoc signed rather than notarized, so the first launch may require right-clicking Plow and choosing **Open**, or allowing it in **System Settings → Privacy & Security**.

For local monitoring, Plow requires a current [Codex CLI](https://learn.chatgpt.com/docs/developer-commands?surface=cli) with managed app-server daemon support. Sessions monitored by Plow should connect to the shared daemon:

```sh
codex --remote unix://
```

To start sessions from Plow, set **Settings → Development home folder** to the absolute path containing your projects. The **Start agent** button can open a general session directly in that development home, or list its immediate subfolders and open a session in a selected project.

Plow first looks for the daemon-capable standalone executable at `$CODEX_HOME/packages/standalone/current/codex` (or `~/.codex/packages/standalone/current/codex`), then checks `PATH` and common install locations. You can override detection from **Settings → Codex executable** with an absolute path; changing it reconnects the monitor without restarting Plow.

An npm or Homebrew installation may run the normal Codex CLI but still be unable to start the managed daemon. In that case, install the standalone build and leave Plow on automatic detection:

```sh
curl -fsSL https://chatgpt.com/codex/install.sh | sh
```

`CODEX_INSTALL_DIR` changes the user-facing command location, while the standalone package used by the daemon remains under `CODEX_HOME`.

On Linux, terminal handoff supports GNOME Terminal, Console (`kgx`), Ptyxis, Konsole, Xfce Terminal, MATE Terminal, Kitty, WezTerm, Foot, Alacritty, Tilix, LXTerminal, xterm, and the standard desktop terminal launchers. Plow tries another installed terminal when a launcher fails immediately.

## SSH hosts

Plow can monitor this computer and multiple SSH hosts at the same time. It uses the system `ssh` command, your existing `~/.ssh/config`, trusted host keys, and SSH agent; it never reads or stores private keys or passwords.

1. Add a concrete alias to `~/.ssh/config` and confirm non-interactive key-based access works:

   ```sshconfig
   Host devbox
     HostName devbox.example.com
     User you
     IdentityFile ~/.ssh/id_ed25519
   ```

   ```sh
   ssh devbox
   ```

2. Install and authenticate the standalone Codex build on the remote host. The `codex` command must be available in the remote login shell, or you can enter its absolute remote path in Plow.

3. Open **Settings → SSH hosts**, add `devbox`, and optionally set its remote development home such as `/home/you/Developer`. Plow will start the remote managed daemon and connect through an SSH stdio proxy; no app-server port is exposed.

Remote project folders appear in **Start agent**. Plow’s terminal and resume buttons open an interactive SSH terminal and connect Codex to that host’s shared daemon. Hosts reconnect independently if a network connection drops. Disable **This computer** in Settings if you only want remote monitoring.

For security, Plow uses OpenSSH batch mode for background monitoring. Password-only hosts will not connect in the background; configure keys and an SSH agent first. Keep app-server off public or shared network listeners, as recommended by the [official Codex remote connection guide](https://learn.chatgpt.com/docs/remote-connections).

## Development

```sh
npm install
npm run dev
```

Browser development uses deterministic demo workers. To run the native monitor, install the Rust stable toolchain and Tauri's platform prerequisites, then run:

```sh
npm run tauri dev
```

Plow starts each configured host's managed Codex app-server daemon and connects to its Unix-socket WebSocket control endpoint, directly for this computer or through `codex app-server proxy` over SSH. Codex sessions should use the shared daemon on the machine where they run:

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
