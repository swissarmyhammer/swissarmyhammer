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

## `--only` hermetic launch mode

`kanban-app --only <board-path>` skips session restore and auto-open and
opens exactly the given board in a single window. UIState persistence is
disabled so the developer's real configuration is untouched. Primarily
intended for scripted launches that need a deterministic starting state.
