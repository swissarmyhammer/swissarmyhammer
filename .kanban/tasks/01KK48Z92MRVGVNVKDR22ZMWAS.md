---
position_column: done
position_ordinal: ffffe480
title: Load actors in frontend EntityStore
---
Add actor entities to the frontend data loading pipeline and add a Tauri command for mention search.

## Changes
- `swissarmyhammer-kanban-app/ui/src/App.tsx` — add `actorEntities` state, fetch in `refresh()`, handle entity events
- `swissarmyhammer-kanban-app/src/commands.rs` — add `search_mentions(entity_type, query)` Tauri command that searches EntityContext by display name/id, returns matches with display_name, color, ID, avatar
- Frontend types — add mention metadata to schema types so entity type info flows to CM6

## Design
- In `refresh()`: add `invoke("list_entities", { entityType: "actor" })` to Promise.all
- Handle `entity-created/removed/field-changed` events for actor entity type
- `search_mentions` command: takes entity_type + query string, searches entities by their `mention_display_field`, returns `[{id, display_name, color, avatar}]`
- Schema context exposes entity type `mention_prefix` and `mention_display_field` to frontend

## Subtasks
- [ ] Add actorEntities state and loading
- [ ] Handle actor entity events (create/remove/field-change)
- [ ] Add search_mentions Tauri command
- [ ] Expose mention metadata in frontend schema types
- [ ] Run `npm test` and `cargo test`