---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffac80
project: task-card-fields
title: Enrich EntityCreated events with computed fields (progress, tags, virtual_tags, filter_tags)
---
## What

`enrich_computed_fields` in `kanban-app/src/commands.rs:2127-2159` only processes `EntityFieldChanged` events — it skips `EntityCreated` events entirely. This means newly created entities arrive at the frontend without computed fields (`progress`, `tags`, `virtual_tags`, `filter_tags`).

### How it happens

1. Watcher detects new entity file → emits `WatchEvent::EntityCreated` with raw disk fields (body, title, etc.) but NO computed fields
2. `enrich_computed_fields` iterates events, matches only `EntityFieldChanged`, skips `EntityCreated`
3. Frontend `handleEntityCreated` (`rust-engine-container.tsx:248-263`) sees non-empty `fields` → uses fast path → adds entity to store WITHOUT computed fields
4. No subsequent `EntityFieldChanged` fires (entity hasn't been modified), so enrichment never runs

### Gap introduced by

Commit `b90138d1e` removed the old enrichment from the event path. Later commits `9e49112ca` and `ba7957081` re-added enrichment but only for `EntityFieldChanged` events, not `EntityCreated`.

### Approach

In `enrich_computed_fields`, also handle `EntityCreated` events. For each `EntityCreated`, read the entity via `ectx.read()` (which runs `derive_all`), run task enrichment if applicable, and merge computed field values into the event's `fields` HashMap.

The pattern mirrors what already exists for `EntityFieldChanged` — read the entity, derive computed fields, append them.

### Subtasks

- [x] Extend the match arm in `enrich_one_watch_event` (`kanban-app/src/commands.rs:2166`) to also handle `EntityCreated` — read entity via `ectx.read()`, run `enrich_task_from_context` for tasks, merge computed fields into `fields` HashMap
- [x] Add Rust test: create a task with checkboxes in body, flush events, verify the `EntityCreated` event's `fields` includes `progress` with correct `{total, completed, percent}` values
- [x] Add frontend test in `rust-engine-container.test.tsx`: `handleEntityCreated` with computed fields in payload correctly populates the entity store

## Acceptance Criteria

- [x] `EntityCreated` events include computed fields (progress, tags, virtual_tags, filter_tags) when emitted to the frontend
- [x] Existing `EntityFieldChanged` enrichment behavior unchanged
- [x] No regressions in `cargo nextest run -p kanban-app` or `npx vitest run`

## Tests

- [x] `kanban-app/src/commands.rs` — test that flushed `EntityCreated` event for a task with GFM checkboxes includes `progress: {total: N, completed: M, percent: P}` in fields
- [x] `kanban-app/ui/src/components/rust-engine-container.test.tsx` — test that `handleEntityCreated` with progress/tags in fields populates entity store correctly
- [x] Full test suites pass: `cargo nextest run -p kanban-app`, `npx vitest run`

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.