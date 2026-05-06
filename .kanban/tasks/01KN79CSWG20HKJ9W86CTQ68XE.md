---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffa580
title: '1. Fix: external file edits don''t update UI — events are signals to re-fetch'
---
## What

External file edits (e.g., editing a `.md` task file in vim) don't cause the UI to update.

### Design principle
Events are **signals to re-fetch**, not data carriers. Eliminate the enrichment path in `flush_and_emit_for_handle` (`commands.rs:1487-1534`). Both command-path and watcher-path should emit the same raw events. The frontend always re-fetches the entity via `get_entity` on any event.

### Root cause
The command path (`flush_and_emit_for_handle`) enriches events with `fullFields` via `ectx.read()`. The frontend short-circuits on `fullFields` present and applies directly. But this creates two different event paths with different behavior. The watcher path emits raw events and the frontend falls back to `get_entity` — which may fail silently.

### Fix approach
1. **Remove enrichment from `flush_and_emit_for_handle`** — stop populating `fields` on events
2. **Frontend always re-fetches** — `entity-field-changed` handler always calls `get_entity`, never applies `fullFields` directly
3. **Simplify `WatchEvent`** — remove the `fields: Option<HashMap>` from `EntityFieldChanged` (or always set to None)
4. **Verify watcher fires for external edits** — ensure the notify crate callback + hash comparison works

### Files to modify
- `kanban-app/src/commands.rs` — remove enrichment block from `flush_and_emit_for_handle` (lines ~1487-1534), remove `cascade_aggregate_events` (computed fields derived on fetch)
- `kanban-app/ui/src/App.tsx` — `entity-field-changed` handler always fetches via `get_entity`, remove `fullFields` short-circuit (lines ~384-400)

## Acceptance Criteria
- [ ] External `.md` file edit → UI updates within watcher debounce period
- [ ] Command-path writes → UI updates (via re-fetch, not enriched event)
- [ ] No enrichment code in `flush_and_emit_for_handle`
- [ ] Frontend always re-fetches on entity events
- [ ] All existing tests pass

## Tests
- [ ] `cargo nextest run --workspace` — no regressions
- [ ] `pnpm test` from `kanban-app/ui/` — UI event handler tests updated
- [ ] Manual: edit task .md externally → UI reflects change
- [ ] Manual: edit task via UI → UI reflects change (re-fetch path)