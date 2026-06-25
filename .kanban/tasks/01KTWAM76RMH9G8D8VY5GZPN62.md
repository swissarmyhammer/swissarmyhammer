---
assignees:
- claude-code
position_column: todo
position_ordinal: f080
project: ui-command-cleanup
title: task.toggleCollapse does not exist — vim `z o` chord dropped during Card J migration; decide owner or delete for good
---
## What

While migrating `SEQUENCE_TABLES` chords into plugin command `keys` (Card J, `01KTED9Z9936CVM6P8YPCZ5WRS`), the vim `z o` → `task.toggleCollapse` entry was found to be DEAD: no plugin, YAML, webview-bus handler, or React `CommandDef` anywhere registers a command with id `task.toggleCollapse` (the only collapse logic is `grouped-board-view.tsx`'s local React `toggleCollapsed` state, which is not command-driven). Pressing `z o` dispatched an id the command service does not know — a guaranteed error/no-op.

Card J therefore dropped the `z o` binding instead of migrating it (the other three chords — `g g`, `g t`/`g Shift+T`, `d d` — moved into nav-commands / perspective-commands / entity-commands as chords).

## Decide

- If collapse-toggle-by-key is wanted: create a real `task.toggleCollapse` (or better-named, e.g. `group.toggleCollapse`) command in the owning plugin, route its execution to the grouped-board-view via the webview command bus, and declare `keys: { vim: "z o" }` (chord schema is now first-class).
- If not wanted: nothing to do — this card just documents why `z o` disappeared.

## Acceptance Criteria
- [ ] Either a working, plugin-registered collapse-toggle command with the `z o` chord and a production-path test, or an explicit decision recorded that the binding stays deleted. #ui