# kanban-app

Tauri v2 desktop binary that hosts the kanban GUI. Rust sources live under
`src/`; the React UI lives under `ui/`.

## Layout

```
kanban-app/
  src/              Tauri backend (Rust)
  ui/               React frontend
  Cargo.toml        kanban-app crate manifest
```

The crate is a workspace member — Rust commands run from the workspace
root; UI commands run from `kanban-app/ui/`.
