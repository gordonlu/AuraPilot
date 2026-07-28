<p align="center">
  <img src="docs/assets/aurapilot-banner.webp" alt="AuraPilot — One task protocol. Any coding agent. All your repositories." width="100%" />
</p>

<p align="center">
  <a href="https://github.com/gordonlu/AuraPilot/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/gordonlu/AuraPilot/actions/workflows/ci.yml/badge.svg" /></a>
  <a href="https://www.rust-lang.org/"><img alt="Rust 1.88+" src="https://img.shields.io/badge/Rust-1.88%2B-000000?logo=rust&logoColor=white" /></a>
  <a href="https://vuejs.org/"><img alt="Vue 3" src="https://img.shields.io/badge/Vue-3-42b883?logo=vuedotjs&logoColor=white" /></a>
  <a href="https://vite.dev/"><img alt="Vite 8" src="https://img.shields.io/badge/Vite-8-646cff?logo=vite&logoColor=white" /></a>
  <a href="https://www.typescriptlang.org/"><img alt="TypeScript 6" src="https://img.shields.io/badge/TypeScript-6-3178c6?logo=typescript&logoColor=white" /></a>
  <a href="https://tauri.app/"><img alt="Tauri 2" src="https://img.shields.io/badge/Tauri-2-24c8db?logo=tauri&logoColor=white" /></a>
  <a href="https://pinia.vuejs.org/"><img alt="Pinia 4" src="https://img.shields.io/badge/Pinia-4-f7d336?logo=pinia&logoColor=111111" /></a>
  <a href="https://vitest.dev/"><img alt="Vitest 4" src="https://img.shields.io/badge/Vitest-4-6e9f18?logo=vitest&logoColor=white" /></a>
  <a href="https://pnpm.io/"><img alt="pnpm 11" src="https://img.shields.io/badge/pnpm-11-f69220?logo=pnpm&logoColor=white" /></a>
  <a href="LICENSE"><img alt="MIT License" src="https://img.shields.io/badge/License-MIT-22c55e.svg" /></a>
  <img alt="Local first" src="https://img.shields.io/badge/Architecture-Local--first-0aa6a6" />
</p>

<p align="center"><strong>One task protocol. Any coding agent. All your repositories.</strong></p>

AuraPilot is a local-first, cross-repository task control plane for AI coding agents.

## Quick start

```sh
cargo install --path crates/aurapilot-cli --locked
aurapilot init /path/to/repository
aurapilot add /path/to/repository
aurapilot status
```

Continue with the [installation guide](docs/INSTALLATION.md) and the one-time [Agent Bootstrap guide](docs/BOOTSTRAP.md).

## Workspace

- `crates/aurapilot-core`: protocol models, validation, path safety, locking, task IDs, and file transactions.
- `crates/aurapilot-cli`: `init`, `add`, and `status` commands shared with the desktop registry.
- `src-tauri`: Tauri 2 desktop shell. Its development identifier is not part of the AuraPilot protocol.
- `web`: Vue 3 + Vite + TypeScript + Pinia frontend, tested with Vitest.
- `prototype`: preserved legacy HTML/CSS/JavaScript interaction prototype. It is reference-only; production frontend code lives in `web/src`.

## Development

Use pnpm exclusively for JavaScript dependencies. The committed `pnpm-lock.yaml` is authoritative.

```sh
pnpm install
pnpm test
pnpm build
cargo test -p aurapilot-core
```

The Bootstrap document is validated only in an isolated repository during Phase 5. It must not be executed against this repository during normal development.
