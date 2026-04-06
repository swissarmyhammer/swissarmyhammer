---
assignees:
- claude-code
depends_on:
- 01KNHCCYJCJB4M1AC42B21PWZ2
- 01KNHCDK7V6R295SA1Z2399YM2
position_column: done
position_ordinal: ffffffffffffffffffffe380
title: Add architecture doc comment to WatchEvent and flush_and_emit_for_handle
---
## What

Add clear architecture documentation to prevent future waffling on the event pattern. Comments should be impossible to misinterpret.

### Files to modify

1. **`kanban-app/src/watcher.rs`** (above `WatchEvent` enum, ~line 27)
   - Add doc comment explaining the two event granularities
   - Explain that `EntityFieldChanged.changes` carries per-field diffs with new values
   - Explain that the watcher produces these diffs from `diff_fields` — no entity reads needed

2. **`kanban-app/src/commands.rs`** (above `flush_and_emit_for_handle`, ~line 1416)
   - Add doc comment explaining the architecture rule
   - Explain the relationship between store events and watcher events
   - Explicitly state: \"DO NOT add EntityContext.read() enrichment here. The watcher's diff_fields produces field-level diffs. See memory: event-architecture.\"

3. **`kanban-app/ui/src/components/rust-engine-container.tsx`** (above event listeners, ~line 200)
   - Add comment explaining the frontend contract:
     - `entity-created`: add entity to store (from payload fields, or get_entity for empty)
     - `entity-field-changed`: patch fields from changes array — ONE path, no branching
     - `entity-removed`: remove from store

## Acceptance Criteria

- [ ] `WatchEvent` enum has a doc comment explaining the two event granularities
- [ ] `flush_and_emit_for_handle` has a doc comment with explicit \"DO NOT enrich\" warning
- [ ] Frontend event listener block has a comment explaining the one-path-per-event contract
- [ ] Comments reference the architecture rule by name (\"event-architecture\")

## Tests

- [ ] No code changes — doc-only card
- [ ] `cargo test --workspace` still passes (no functional changes)
- [ ] `cd kanban-app/ui && npx vitest run` still passes

## Workflow
- Implement directly — no TDD needed for doc-only changes. #events