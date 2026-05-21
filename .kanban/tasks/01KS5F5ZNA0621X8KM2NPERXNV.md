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

Establish and enforce the invariant that the `kanban`, `views`, and `store` MCP servers (and any future store-backed server) share **one** `Arc<StoreContext>` — the single undo substrate. This already holds in production (`apps/kanban-app/src/state.rs:281` creates one `StoreContext`; entities register via `EntityContext::set_store_context` + `register`, views/perspectives via `PerspectiveContext::set_store_context`/`ViewsContext`). The new server architecture must preserve it: each server is constructed with the *same* `Arc<StoreContext>`, never its own.

This task is the wiring + guardrail, not new undo logic. Undo/redo logic already lives in `swissarmyhammer-store::StoreContext` (verified: `context.rs::undo/redo`, `stack.rs::UndoStack`, `changelog.rs` forward/reverse patches). Undo is exposed to callers by the `store` MCP server (separate task), NOT by `app` — `app` is quit/about/help only.

Files:
- `apps/kanban-app/src/state.rs` (and the host bootstrap) — construct one `Arc<StoreContext>`; pass the same Arc to the `kanban`, `views`, and `store` server constructors. The `store` server's undo/redo operate on it; the `kanban`/`views` servers write through stores registered into it.
- Document the substrate principle near the construction site: "one StoreContext, one undo_stack.yaml; all store-backed servers share it; servers never construct their own."

## Acceptance Criteria
- [ ] Exactly one `StoreContext` is constructed for the running app; `kanban`, `views`, and the `store` server all hold that same `Arc`
- [ ] Entity stores (task/column/tag/project/actor) AND view/perspective stores are all registered into that one `StoreContext`
- [ ] `store.undo` reverts the most recent write regardless of which server produced it (entity, view, or perspective), in one LIFO stack
- [ ] There is no second `StoreContext` / second `undo_stack.yaml` anywhere in the production path

## Tests
- [ ] `crates/swissarmyhammer-command-service/tests/integration/shared_undo_substrate_e2e.rs` — boot the host; perform (a) an entity edit via `kanban`, (b) a perspective filter change via `views`, (c) a clipboard paste via `entity`/`kanban`; assert all three appear on ONE undo stack; `store.undo` x3 reverts them newest-first across the servers (`UndoOutcome.items` identifies which store each entry touched)
- [ ] A guard test asserts only one `StoreContext` instance backs the registered servers (e.g. pointer-equality of the `Arc` handed to each server, or a single-construction assertion in `for_tests` wiring)
- [ ] `cargo test -p swissarmyhammer-command-service --test integration shared_undo_substrate_e2e` passes

## Workflow
- Use `/tdd` — write the cross-server single-stack test first; it pins the substrate invariant.

Prerequisite for the `store` server (undo/redo) and the undo-grouping + reconciliation tasks. Depends on the operation-struct foundation.