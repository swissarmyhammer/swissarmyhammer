---
position_column: done
position_ordinal: 8d80
title: Rename kanban-app binary from "kanban" to "kanban-app"
---
## What
Rename the kanban-app binary to free the `kanban` name for the new CLI.

Change `kanban-app/Cargo.toml` line 12: `name = "kanban"` → `name = "kanban-app"`

## Acceptance Criteria
- [ ] `cargo build -p kanban-app` produces a binary named `kanban-app`, not `kanban`
- [ ] No other workspace members reference the old binary name

## Tests
- [ ] `cargo build -p kanban-app` succeeds
- [ ] Verify `target/debug/kanban-app` exists