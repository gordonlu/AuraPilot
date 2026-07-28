# AuraPilot installation

AuraPilot v0.1 ships as two local components:

- `aurapilot`: the repository initialization and status CLI;
- `aurapilot-desktop`: the Tauri desktop control plane.

Neither component installs an Agent plugin, modifies global Agent configuration, or runs a background Agent service.

## Prerequisites for source builds

- Rust 1.85 or newer;
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
aurapilot status
```

Windows PowerShell:

```powershell
aurapilot init "C:\Users\user\code\my project" --owner user
aurapilot add "C:\Users\user\code\my project"
aurapilot status
```

`init` creates `.aurapilot/AGENTS.md`, `project.yaml`, `schema.json`, `installation.yaml`, and the four task-state directories. Existing protocol files and tasks are preserved.

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
