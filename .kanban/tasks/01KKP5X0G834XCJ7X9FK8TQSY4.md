---
position_column: done
position_ordinal: '9980'
title: Command palette search mode
---
## What
Add a `mode` prop to `CommandPalette` ("command" | "search"). In search mode, the palette calls `invoke("search_entities", { query, limit })` (the Rust-side Tauri command backed by EntitySearchIndex) and displays results with entity type + display name.

Each search result row is wrapped in a `FocusScope` with an `entity.inspect` command registered — the same pattern used by `entity-card.tsx`, `tag-pill.tsx`, `column-view.tsx`, etc. Selecting a result sets focus and dispatches `entity.inspect` through the scope chain. This means search results are first-class entities in the command system: they get focus, context menus, and inspect all works through normal dispatch.

**Files:** `kanban-app/ui/src/components/command-palette.tsx`

**Approach:**
- Add `mode` prop defaulting to "command"
- In search mode, debounce input (150ms) and call `invoke("search_entities", { query, limit: 50 })`
- Results come back as `[{ entity_type, entity_id, display_name, score }]`
- Each result row is a `FocusScope` with moniker `"type:id"` and commands `[{ id: "entity.inspect", execute: () => inspect(moniker) }]`
- Show entity type as a muted prefix, display_name as main text (no field captions)
- On Enter or click, the FocusScope's `entity.inspect` fires through the scope chain
- Arrow keys / j/k change selection (and could set focus to that result's FocusScope)
- Placeholder text changes to "Search..."
- Empty query shows nothing (or a "type to search" hint)

## Acceptance Criteria
- [ ] Search results are wrapped in FocusScope (participate in command system)
- [ ] Selecting a result dispatches entity.inspect through scope chain
- [ ] Search mode calls Rust-side search via Tauri invoke
- [ ] Debounced input (150ms) prevents excessive calls
- [ ] Results show entity type + display name
- [ ] Escape closes search palette
- [ ] Vim/CUA/emacs keymap modes work in search input

## Tests
- [ ] Existing command palette tests still pass
- [ ] Manual: search, select result, inspector opens via entity.inspect dispatch