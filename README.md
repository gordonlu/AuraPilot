# AuraPilot

**One task protocol. Any coding agent. All your repositories.**

AuraPilot is a local-first, cross-repository task control plane for AI coding agents.

## Workspace

- `crates/aurapilot-core`: protocol models, validation, path safety, locking, task IDs, and file transactions.
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
