---
assignees:
- claude-code
position_column: todo
position_ordinal: a480
project: store-service
title: Substrate-guard test should call into production wiring, not mirror it
---
## What

`apps/kanban-app/tests/substrate_guard.rs` re-implements the `BoardHandle::open` wiring sequence in the test body (its own `register_entity_stores` / `register_perspective_store` / `register_view_store` calls). That means a future change to the actual production wiring that splits the `StoreContext` would NOT cause this test to fail — the test runs its own (still-correct) wiring against the contexts. This was a deliberate compromise during the Tier 0 card because kanban-app is a `[[bin]]` crate with no library target, but it weakens the guard the card claims to install.

## Acceptance Criteria

- [ ] Extract the substrate-wiring sequence into a thin helper (e.g. a `pub(crate) fn wire_board_substrate(...)` in a new `kanban-app` lib target, or push it into `swissarmyhammer-kanban` next to the `KanbanContext` open path)
- [ ] `BoardHandle::open` calls the helper instead of inlining the three `register_*` calls
- [ ] `substrate_guard.rs` calls the SAME helper, so the test and production share one code path; any future fork in the substrate breaks the test directly
- [ ] Test still asserts `Arc::ptr_eq` across all three contexts

## Notes

- Discovered during review of `01KS5F5ZNA0621X8KM2NPERXNV`.
- Today the kanban-app `[bin]` target is the obstacle. Adding a `[lib]` to the crate (so the helper is callable from `tests/`) is the most pragmatic fix; alternatively, the wiring helper can live in `swissarmyhammer-kanban` since that crate already owns `KanbanContext::open`.