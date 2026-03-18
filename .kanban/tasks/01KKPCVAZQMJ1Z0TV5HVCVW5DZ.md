---
position_column: done
position_ordinal: f180
title: Fix stale doc comment on search_entities
---
## What
The doc comment on `search_entities` in `commands.rs` still says "the frontend command palette performs its own client-side fuzzy search". This is no longer true — the frontend now calls this IPC command. Remove the stale paragraph.

**File:** `kanban-app/src/commands.rs` lines 350-353

## Acceptance Criteria
- [ ] Doc comment accurately describes current behavior