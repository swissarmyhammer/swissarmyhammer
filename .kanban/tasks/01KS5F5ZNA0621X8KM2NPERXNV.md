---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: '9980'
project: store-service
title: 'Shared StoreContext substrate: single undo stack across all command-backing servers'
---
## What

Pin the **single-undo-substrate invariant** that production already obeys: there is exactly one `Arc<StoreContext>` (built in `BoardHandle::open` at `apps/kanban-app/src/state.rs:~323`) and one `undo_stack.yaml`; every `TrackedStore` — entity stores (task/column/tag/project/actor) AND view + perspective stores — registers into that one context via `Arc::clone(&store_context)`, so `undo`/`redo` revert across all of them on one LIFO stack.

This task is the **documentation + regression guard** for that invariant. The wiring already exists (`register_entity_stores` / `register_perspective_store` / `register_view_store` all take the same `Arc`); what's missing is (a) an explicit doc comment at the construction site naming the invariant, and (b) a test that fails loudly if a future change accidentally introduces a second `StoreContext` or a second `undo_stack.yaml`.

This card is intentionally Tier 0 / standalone. The cross-server end-to-end test of `store.undo` reverting entity + view + perspective edits through the new MCP face is a **separate, deferred concern**: it requires the `store`, `kanban`, `views`, and `entity` MCP servers to exist (none do yet), so it belongs in `command-events` or `command-cutover`, not here.

Files:
- `apps/kanban-app/src/state.rs` — add a doc comment at the `BoardHandle::open` construction site (currently ~line 323) stating: "one `Arc<StoreContext>` per app; every `TrackedStore` registers into it; never construct a second one — that would fork the undo stack."
- `crates/swissarmyhammer-store/src/lib.rs` (or `context.rs` module docs) — record the same invariant near `StoreContext` so a reader of the store crate sees the contract.

## Acceptance Criteria
- [ ] Doc comment at the `BoardHandle::open` construction site names the invariant in plain English
- [ ] `StoreContext` module docs reinforce "one per app; share via `Arc::clone`; never construct a second"
- [ ] Guard test proves: opening the production-shape board state yields a single `StoreContext`, and the `Arc`s the entity/view/perspective registrations hold all `Arc::ptr_eq` to the one the `BoardHandle` exposes
- [ ] No code change to the substrate wiring itself — it is already correct in production

## Tests
- [ ] `apps/kanban-app/tests/substrate_guard.rs` — open a `BoardHandle` against a temp `.kanban` dir; obtain the `store_context` Arc; pull each registered `TrackedStore`'s context Arc (entity-stores via `EntityContext`, view-store via `ViewsContext`, perspective-store via `PerspectiveContext`); `Arc::ptr_eq` each against the `BoardHandle`'s. A failure here means somebody constructed a second `StoreContext`.
- [ ] `cargo test -p kanban-app --test substrate_guard` passes

## Workflow
- Use `/tdd` — write the guard test first; against the current code it passes (because the substrate is already correct). The point is that it FAILS the moment a future change splits the substrate.

Standalone: the substrate already exists in production. This card touches no other crate, depends on nothing, and does NOT require the `store` MCP card (`01KS5F7BR6850RKT67X4CNHPAZ`). The cross-server e2e undo test was removed from scope — it lives in `command-events`/`command-cutover` where the necessary MCP servers exist.