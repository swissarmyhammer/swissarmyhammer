---
position_column: done
position_ordinal: ffffde80
title: Auto-create OS user actor on board open
---
Use `whoami` crate to automatically create an actor for the logged-in OS user when a board is opened.

## Changes
- `swissarmyhammer-kanban-app/Cargo.toml` — add `whoami` dependency
- `swissarmyhammer-kanban/src/actor/add.rs` — extend `AddActor` to accept optional `color` and `avatar` fields
- `swissarmyhammer-kanban-app/src/state.rs` — add `ensure_os_actor()` called after `BoardHandle::open()`

## Design
- Actor ID = `whoami::username()`, display name = `whoami::realname()`
- Derive deterministic hex color from username hash (mod a curated palette)
- Generate initials-based SVG avatar as data URI
- Try to read macOS profile picture from filesystem, prefer real photo over generated
- Use `AddActor::human(username, realname).with_ensure()` — idempotent on re-open

## Subtasks
- [ ] Add `whoami` to Cargo.toml
- [ ] Extend AddActor with optional color/avatar fields
- [ ] Implement `ensure_os_actor()` in state.rs
- [ ] Generate deterministic color from username hash
- [ ] Generate initials SVG avatar (or read macOS profile picture)
- [ ] Call ensure_os_actor after BoardHandle::open()
- [ ] Run `cargo test`