---
assignees:
- claude-code
position_column: todo
position_ordinal: '9080'
title: Move perspective commands out of swissarmyhammer-commands crate into swissarmyhammer-perspectives
---
## What

Perspective commands currently live in the generic `swissarmyhammer-commands` crate and kanban-kanban crate's commands module. They belong in `swissarmyhammer-perspectives` — the crate whose domain they implement.

The user's words: "I don't get why the perspective commands are in the command crate instead of the perspective crate — makes no fucking sense".

This is the same organizational principle as the companion card (entity.yaml reorg): domain-specific commands belong with the domain. A generic commands crate should only host generic orchestration (dispatch, registry, command trait). The perspective domain has 15+ commands — `perspective.load`, `perspective.save`, `perspective.delete`, `perspective.rename`, `perspective.filter`, `perspective.clearFilter`, `perspective.group`, `perspective.clearGroup`, `perspective.list`, `perspective.sort.set`, `perspective.sort.clear`, `perspective.sort.toggle`, `perspective.next`, `perspective.prev`, `perspective.goto` — and their ops and YAML declarations should all be co-located in `swissarmyhammer-perspectives`.

## Scope

1. **Move command impls** — `swissarmyhammer-kanban/src/commands/perspective_commands.rs` → `swissarmyhammer-perspectives/src/commands.rs` (or a `commands/` module there).
2. **Move YAML declarations** — wherever perspective.* commands are declared (`swissarmyhammer-commands/builtin/commands/` — e.g. `perspective.yaml` if it exists). Move to `swissarmyhammer-perspectives/builtin/commands/perspective.yaml` and have the perspectives crate contribute its YAML via `include_dir!`.
3. **Registration wiring** — `register_perspective` currently lives in `swissarmyhammer-kanban/src/commands/mod.rs:168`. After move, the perspectives crate should export a `register` fn that the kanban crate calls (keeping the aggregation at the app layer, but the definitions in their home crate).
4. **Sibling crates / reuse** — if anything outside kanban uses these perspective commands, make sure the new crate location is reachable.

## Acceptance Criteria

- [ ] `swissarmyhammer-perspectives/src/commands.rs` contains all `LoadPerspectiveCmd`, `SavePerspectiveCmd`, …, `GotoPerspectiveCmd` impls.
- [ ] `swissarmyhammer-kanban/src/commands/perspective_commands.rs` deleted or reduced to a thin re-export.
- [ ] `register_perspective` in `swissarmyhammer-kanban/src/commands/mod.rs` calls `swissarmyhammer_perspectives::commands::register(&mut map)` or equivalent.
- [ ] All perspective-related YAML lives under `swissarmyhammer-perspectives/builtin/`.
- [ ] No regression in behavior — all perspective commands dispatch identically.
- [ ] `cargo build` clean, `cargo clippy -- -D warnings` clean.
- [ ] Existing perspective tests still pass.

## Tests

- [ ] `swissarmyhammer-perspectives` gains unit tests for the migrated command impls (they existed before in kanban).
- [ ] `cargo nextest run -p swissarmyhammer-perspectives -p swissarmyhammer-kanban` all pass.

## Workflow

- Create a stub `commands` module in perspectives first, move one command as a pilot, confirm build + tests pass, then mass-move the rest.

#organization