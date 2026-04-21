---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff8380
project: spatial-nav
title: 'Enter-to-activate: bind Enter on board cards (inspect) and perspective tabs (switch)'
---
## What

Establish "Enter = activate the focused scope" consistently across all focusable scopes. Each scope type has its own idiomatic activation:

| Scope type | Enter action | Status |
|---|---|---|
| LeftNav button | `view.switch:<id>` | ✅ done (`left-nav.tsx:168-178`, `view.activate.<id>`) |
| Perspective tab | switch perspective (`setActivePerspectiveId(p.id)`) | ✅ done (`perspective-tab-bar.tsx:296-307`, `perspective.activate.<id>`) |
| Grid body cell | `grid.edit` / `grid.editEnter` (enter edit mode) | ✅ done (`grid-view.tsx:271-272`) |
| Inspector field row | `inspector.edit` / `inspector.editEnter` | ✅ done (`inspector-focus-bridge.tsx:39-48`) |
| Row selector | `ui.inspect` with target | ✅ done (`data-table.tsx:1120-1132`) |
| Board / entity card | `ui.inspect` with target | ✅ done (`entity-card.tsx:297-336`, `entity.activate.<moniker>`) |

## Implementation notes

### Gap 1 — Perspective tabs (done)

`ScopedPerspectiveTab` now builds a per-tab `commands` array with a single namespaced command `perspective.activate.<id>` bound to `Enter` across all keymaps. Execute calls the existing `onSelect` (which calls `setActivePerspectiveId`). The `FocusScope` receives this via `commands={commands}` so pressing Enter while a tab is spatially focused dispatches the same handler as a click.

### Gap 2 — Board / entity cards (done, Option B)

The task text flagged Option A (adding `keys` to `ui.inspect` in `swissarmyhammer-commands/builtin/commands/ui.yaml`) as preferred, but audit showed that YAML feeds the `CommandsRegistry` (used by `scope_commands.rs` for palette/context-menu resolution) — not the entity-level `commands` serialized by `get_entity_schema`. Entity commands load straight from each entity's own YAML (`EntityDef.commands`) and the frontend's `useEntityCommands` reads those verbatim, so keys added at the registry level never reach the card's FocusScope bindings.

Switched to **Option B**: `EntityCard` now builds a per-instance command via `useEnterInspectCommand(moniker)` and passes it through the existing `extraCommands` merge slot into `useEntityCommands`. The command uses a namespaced id (`entity.activate.<moniker>`) so it does not shadow the schema-derived `ui.inspect` entry inside the card's scope `Map` (`contextMenu: false`, so the right-click menu still shows a single Inspect entry from the schema). Its `execute` calls `dispatchInspect({ target: moniker })` — the exact same dispatch shape as the `(i)` `InspectButton`, so keyboard and mouse activation converge.

## Acceptance Criteria

- [x] Focus a perspective tab via spatial nav, press Enter → the active perspective switches to that tab
- [x] Focus a board / entity card via spatial nav, press Enter → inspector opens for that card's entity
- [x] Focus a grid cell, press Enter → edit mode activates (existing — regression-protected)
- [x] Focus an inspector field row, press Enter → field enters edit mode (existing — regression-protected)
- [x] Focus a row selector, press Enter → inspector opens for that row's entity (existing — regression-protected)
- [x] Focus a LeftNav button, press Enter → view switches (existing — regression-protected)
- [x] No accidental double-fires (namespaced command ids prevent sibling shadowing; `contextMenu: false` prevents duplicate menu entries)

## Tests

- [x] `kanban-app/ui/src/components/perspective-tab-bar.test.tsx` — new test "pressing Enter on a focused tab switches to that perspective" uses `FixtureShell` to drive the production `createKeyHandler` → `extractScopeBindings` pipeline; asserts `setActivePerspectiveId` was called with the focused tab's id
- [x] `kanban-app/ui/src/components/entity-card.test.tsx` — new test "pressing Enter on a focused card dispatches ui.inspect with the card's moniker" uses `FixtureShell` + a `FocusSetter` harness (programmatic focus avoids triggering inner field edit mode); asserts `dispatch_command` invoked with `cmd: "ui.inspect"` and `target: "task:task-1"`
- [x] Regression: all 1362 frontend tests pass (previously 1360; +2 new)
- [x] Regression: 175 `swissarmyhammer-commands` Rust tests pass (no YAML change shipped, so backend is untouched)
- [x] `tsc --noEmit` clean

## Workflow

- Used `/tdd` — wrote failing tests first, then implemented.
- Started with Gap 1 (perspective tabs) — mirrored the `ViewButton` / `view.activate.<id>` pattern one-for-one.
- For Gap 2, audited whether Option A (YAML) would actually flow to the frontend; discovered the registry YAML does not merge into entity commands, so fell back to Option B with an explicit rationale in `entity-card.tsx`.

## Follow-ups (not in this task)

- `01KPQXEMJEGVY7JF9HM5JSWTAP` (LeftNav Enter — already implemented in code) can be closed.
- `01KPRGXFE2EE2SQZQB2PR63N2X` (row selector Enter — already implemented in code) can be closed.
- Column header Enter → sort remains unfiled.
