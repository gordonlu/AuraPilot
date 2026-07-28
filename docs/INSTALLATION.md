# AuraPilot installation

AuraPilot v0.1 ships as two local components:

- `aurapilot`: the repository initialization, task creation, and status CLI;
- `aurapilot-desktop`: the Tauri desktop control plane.

Neither component installs an Agent plugin, modifies global Agent configuration, or runs a background Agent service.

## Prerequisites for source builds

- Rust 1.88 or newer;
- Node.js 22;
- pnpm 11.9.0;
- the platform dependencies required by Tauri 2.

Install JavaScript dependencies once:

```sh
pnpm install --frozen-lockfile
```

## Install the CLI

From the repository root:

```sh
cargo install --path crates/aurapilot-cli --locked
aurapilot --version
```

The CLI and desktop application share `~/.aurapilot/config.json`. This path is independent of the temporary Tauri development identifier.

### Initialize a repository

Linux:

```sh
aurapilot init "/home/user/code/my project" --owner user
aurapilot add "/home/user/code/my project"
aurapilot task create "/home/user/code/my project" --title "Improve Push flow" --priority P1 --type feature --accept "User can choose a branch strategy"
aurapilot status
```

Windows PowerShell:

```powershell
aurapilot init "C:\Users\user\code\my project" --owner user
aurapilot add "C:\Users\user\code\my project"
aurapilot task create "C:\Users\user\code\my project" --title "Improve Push flow" --priority P1 --type feature --accept "User can choose a branch strategy"
aurapilot status
```

`init` creates `.aurapilot/AGENTS.md`, `project.yaml`, `schema.json`, `installation.yaml`, and the four task-state directories. Existing protocol files and tasks are preserved.

`task create` creates the next available backlog task atomically. The repository path defaults to the current directory; `--priority` defaults to `P1`, `--type` defaults to `feature`, and `--accept` can be repeated:

```sh
aurapilot task create --title "Improve Push flow" \
  --desc "Let the user choose the branch strategy" \
  --accept "The choice is explicit" \
  --accept "Push does not switch branches"
```

AuraPilot protocol data is tracked by Git by default. Pass `--ignore` only when the repository owner explicitly wants `.aurapilot/` added to `.gitignore`:

```sh
aurapilot init . --ignore
```

After initialization, give [the repository Bootstrap guide](BOOTSTRAP.md) to the current Coding Agent once. It adds one minimal repository-level reference without changing global configuration, running tasks, or committing files.

## Run the desktop application

Development:

```sh
pnpm tauri dev
```

If an AuraPilot Vite server is already running on port 28727 (`AURAP` on a phone keypad), the desktop launcher reuses it. If another application owns the port, startup stops with a short diagnostic instead of a Vite stack trace.

Build the desktop binary and native installer for the current platform:

```sh
pnpm tauri build
```

Supported Phase 5 packaging targets:

- Linux: AppImage and Debian package;
- Windows: NSIS and MSI installers.

The GitHub `Build` workflow creates the CLI binary and native desktop bundles on both operating systems for version tags and manual workflow runs.

## Safety defaults

- `.aurapilot/` is the only task-protocol source of truth;
- no automatic `git add`, commit, push, branch switch, or global configuration change;
- repository roots may be symbolic links, but links inside `.aurapilot/` cannot escape the repository;
- paths with spaces, Unicode, and Windows PowerShell path syntax are accepted as ordinary path arguments;
- Bootstrap must be idempotent and must preserve content outside its marker block.
