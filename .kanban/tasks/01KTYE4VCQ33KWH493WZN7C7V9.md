---
assignees:
- claude-code
position_column: todo
position_ordinal: f280
title: 'Perspective cache never refreshes from disk — entity watcher rejects perspective files ("unknown entity type: perspective"), so cross-window rename/delete see stale perspective.list'
---
## Symptom

`perspective.list` (and therefore the perspective tab bar) goes stale when another window or sibling process renames or deletes a perspective. Each process loads its `PerspectiveContext` once and never re-reads `.kanban/perspectives/` — an external rename/delete is invisible until the board is re-opened. Only the default-CREATE path is convergence-safe today (deterministic `default-<scope>` ids from card 01KTY6T1GPY94VYWANE9X41SKJ make duplicate creates upsert the same file); UPDATE/RENAME/DELETE still operate on stale in-memory state.

## Log evidence

The entity watcher (`crates/swissarmyhammer-entity/src/watcher.rs`) parses `.kanban/perspectives/<id>.yaml` into entity type `perspective` and calls `EntityCache::refresh_from_disk_with("perspective", ...)`, which fails with the `EntityError::UnknownEntityType` message:

```
unknown entity type: perspective
```

(`crates/swissarmyhammer-entity/src/error.rs` — `#[error("unknown entity type: {entity_type}")]`). The fields/entity registry has no `perspective` entity definition, so `EntityContext::entity_def("perspective")` errors and the watcher event is dropped — no cache invalidation ever fires for perspective files. Observable in the unified log: `log show --predicate 'subsystem == "com.swissarmyhammer.kanban"' | grep 'unknown entity type: perspective'`.

## Routed around, not fixed

Card 01KTY6T1GPY94VYWANE9X41SKJ deliberately routed AROUND this gap instead of fixing it: deterministic ensure ids + board-open reconciliation make stale-cache duplicate CREATEs harmless, and the module doc of `crates/swissarmyhammer-kanban/src/perspective/ensure_default.rs` explicitly records "the entity watcher rejects perspective files with \"unknown entity type\"". The underlying staleness for list/rename/delete remains.

## What a fix needs

- The entity watcher pipeline must learn the `perspective` entity type so cache invalidation fires for `.kanban/perspectives/*.yaml`: either register a `perspective` entity definition (so `EntityCache::refresh_from_disk_with` resolves it) or add a dedicated watcher route that reloads/evicts the `PerspectiveContext` (`KanbanContext::perspectives`) on file events — the invalidation must reach the `Arc<RwLock<PerspectiveContext>>` that `perspective.list`/rename/delete read.
- Alternative (acceptable fallback): reload-on-read — `perspective_context()` revalidates against disk (mtime/dir-generation check) before serving.
- A real-pipeline test: process A renames/deletes a perspective on disk; process B's (or a second context's) `perspective.list` reflects it without re-opening the board.
- Frontend store must receive the resulting change notification so the tab bar re-renders (store-event-loop rule).

## Constraints

- Crate-scoped builds/tests only (`-p swissarmyhammer-kanban`, `-p swissarmyhammer-entity`).
- Use tracing, never eprintln. #ui