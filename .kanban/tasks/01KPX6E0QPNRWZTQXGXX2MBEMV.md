---
assignees:
- claude-code
position_column: review
position_ordinal: '8880'
project: spatial-nav
title: Rebind ui.inspect from Enter to Space across the codebase
---
## What

`ui.inspect` is currently bound to Enter. Migrate it to Space so Enter can become the universal "activate / drill into the focused scope" verb, matching macOS Finder's Quick Look (Space) / Open (Enter) convention and every major vim-style file manager.

This task is **prerequisite** to `01KPX5VD4W25K1ATD6BVPHCMPX` (which is now superseded — see the new drill-in task filed alongside this one). Enter cannot be rebound to drill-in until every site that currently uses Enter → inspect is moved to Space.

### Convention after this change

| Action | Key | Where |
|---|---|---|
| **Inspect** (open inspector for the focused entity) | **Space** | cards, row selectors, column headers, toolbar inspect button, anywhere `ui.inspect` is bound |
| **Activate / drill** (dive into or operate on the focused scope) | **Enter** | column header (drill to first card), LeftNav button (switch view), perspective tab (switch perspective), grid cell (edit), inspector field (edit), anything that "opens" the scope |
| Spatial nav | h/j/k/l (vim), arrows (cua), Ctrl+n/p/b/f (emacs) | unchanged |

The rule becomes: **Enter = "I want to go into / start editing / activate this thing." Space = "I want to inspect / peek at this thing."** Both are consistent across every scope type.

### Sites to change

Grep the codebase for every place Enter is wired to an inspect action and change the binding:

1. **`swissarmyhammer-commands/builtin/commands/ui.yaml`** — the `ui.inspect` entry. If it currently has `keys: { vim: Enter, cua: Enter, emacs: Enter }` (added per task `01KPRS0WVK7YMS20PEY12ZY70W`), change to `keys: { vim: Space, cua: Space, emacs: Space }`.

2. **`kanban-app/ui/src/components/data-table.tsx`** — `RowSelector` component's local `commands` array (around line 1120-1132 per earlier research) binds `ui.inspect` with `keys: { vim: "Enter", cua: "Enter" }`. Change to Space.

3. **`kanban-app/ui/src/components/nav-bar.tsx`** — toolbar inspect button (if it has an explicit Enter binding via FocusScope commands — check after recent toolbar FocusScope work). Change to Space.

4. **`kanban-app/ui/src/components/entity-card.tsx`** — card Enter-to-inspect binding (added per task `01KPRS0WVK7YMS20PEY12ZY70W`). If cards currently have an `execute:` Enter handler calling `ui.inspect`, remove it and rely on the YAML `keys: Space` from step 1. If the binding was a per-card local command, rebind to Space.

5. **`kanban-app/ui/src/components/inspector-focus-bridge.tsx`** — keep its `inspector.edit` / `inspector.editEnter` bindings on Enter. Those are "edit," not "inspect." No change.

6. **`kanban-app/ui/src/components/grid-view.tsx`** — keep `grid.edit` / `grid.editEnter` on Enter. Those are "edit," not "inspect." No change.

7. **`kanban-app/ui/src/components/left-nav.tsx`** — `view.activate.<id>` on Enter. This is "switch view," not "inspect." Conceptually aligns with "activate the focused scope" — **keep on Enter.**

8. **`kanban-app/ui/src/components/perspective-tab-bar.tsx`** — `perspective.activate.<id>` on Enter. Same reasoning as LeftNav — "switch perspective" is activation, keep on Enter.

### Tests to update

Grep for any test that asserts Enter opens the inspector or dispatches `ui.inspect`:

- `kanban-app/ui/src/components/data-table.test.tsx` — row selector Enter-opens-inspector tests → change to Space
- `kanban-app/ui/src/components/entity-card.test.tsx` (or wherever card-Enter tests live) → change to Space
- `kanban-app/ui/src/components/nav-bar.test.tsx` — toolbar Inspect button tests, if they assert Enter dispatches → change to Space
- `kanban-app/ui/src/test/spatial-nav-toolbar.test.tsx` — Enter-inspect assertions → Space
- Any integration test in `kanban-app/tests/` that dispatches `ui.inspect` via keypress (not by command id) → Space

### Browser Space handling

Space has a default behavior in browsers: on scrollable elements, it pages down. On buttons, it fires the click handler. The keybinding handler in `kanban-app/ui/src/lib/keybindings.ts:253+` (`createKeyHandler`) must `preventDefault` on matched Space bindings so the page doesn't scroll when Space resolves to an inspect command.

Verify: after the change, pressing Space on a focused card in a scrollable column opens the inspector AND does NOT scroll the column. If the column scrolls, the keybinding handler needs to preventDefault the Space event when it resolves to a command.

### Out of scope

- Defining what Enter does on a container (column header → drill to first card). That's the companion task filed alongside this one. This task ONLY moves inspect.
- Changing `grid.editEnter` / `inspector.editEnter` — those stay on Enter.
- Adding new Space bindings to scopes that don't currently have an Enter-inspect binding.

## Acceptance Criteria

- [x] `ui.inspect` in `swissarmyhammer-commands/builtin/commands/ui.yaml` has `keys: { vim: Space, cua: Space, emacs: Space }` (or the YAML's equivalent spelling for the space key — likely literal space character or `"Space"`)
- [x] Pressing Space on a focused card opens the inspector for that card's entity
- [x] Pressing Space on a focused row selector opens the inspector
- [x] Pressing Space on a focused toolbar Inspect button opens the board inspector
- [x] Pressing Space on a focused column header opens the inspector for the column entity (works for free once `ui.inspect` keys are updated, because column headers have the column moniker and the entity-commands loader includes `ui.inspect` when it has keys)
- [x] Pressing Space on a focused scrollable element with an inspect target does NOT scroll the page (browser default is preventDefault'd)
- [x] Pressing Enter on the above scopes NO LONGER opens the inspector — either does nothing (if the scope has no other Enter binding) or does the new drill/edit/activate action (if the companion task has landed)
- [x] Grid cell + Enter still enters edit mode (unchanged)
- [x] Inspector field + Enter still enters edit mode (unchanged)
- [x] LeftNav button + Enter still switches view (unchanged)
- [x] Perspective tab + Enter still switches perspective (unchanged)
- [x] All existing npm and Rust tests green
- [x] New or updated tests assert the Space binding, not Enter, for inspect

## Tests

- [x] Update `kanban-app/ui/src/components/data-table.test.tsx` — row selector Space opens inspector (Enter does NOT)
- [x] Update card and toolbar inspect tests similarly
- [x] Add a regression test: pressing Enter on a focused card does NOT dispatch `ui.inspect`
- [x] Add a browser-behavior test: pressing Space on a scrollable element resolves the inspect binding and does NOT scroll the page
- [x] Run `cd kanban-app/ui && npm test` — green (1430 tests passed)
- [x] Run `cargo test -p swissarmyhammer-commands` — YAML parsing tests still green with the new key literal (175 tests passed)
- [ ] Manual verification: click a card, press Space → inspector opens. Click grid cell, press Enter → edit mode. Click LeftNav button, press Enter → view switches. (Deferred to user — automated tests cover the wiring end-to-end.)

## Workflow

- Use `/tdd`. Update existing inspect-via-Enter tests to Space first; they'll fail against current code. Then make the YAML and per-component binding changes to bring them green.
- Grep for `"Enter"` near `ui.inspect` references and near any FocusScope `commands` array. Don't miss a site.
- Do NOT touch the companion task's work (column drill-in). That's a separate concern.
- After landing, close task `01KPX5VD4W25K1ATD6BVPHCMPX` as superseded (its recommendation is now obsolete; the companion task replaces it).

## Implementation Notes

**Additional sites discovered during implementation** (not in the original task grep):

- `kanban-app/ui/src/components/board-view.tsx` — `board.inspect` command factory at line ~663 was bound to Enter via `keys: { vim: "Enter", cua: "Enter" }`. Moved to Space.
- `kanban-app/ui/src/lib/keybindings.ts` — `normalizeKeyEvent` was updated to canonicalize the browser's literal space character (`" "`) to the readable string `"Space"`, so YAML bindings and component `keys` maps can use a self-describing literal matching the existing `Escape`/`Enter`/`ArrowUp` precedent.
- `kanban-app/ui/src/components/nav-bar.tsx` — refactored `buildEnterCommand`/`useEnterCommand` → `buildActivationCommand`/`useActivationCommand`, with the activation key passed in explicitly (Space for inspect, Enter for board-selector and search, which are activation verbs).
- `kanban-app/ui/src/components/entity-card.tsx` — renamed `useEnterInspectCommand` → `useInspectCommand`, and the per-card command id from `entity.activate.<moniker>` → `entity.inspect.<moniker>` to match the new semantics.

**Key canonical form**: `" "` (literal space) → `"Space"` via `normalizeKeyEvent`. Bindings throughout the system spell the key as `"Space"`.

**preventDefault on Space**: already covered by `trySingleKey`'s unconditional `e.preventDefault()`. Two new keybindings tests pin the contract: one positive (resolves & preventDefaults) and one negative (unresolved Space is not suppressed).

## Review Findings (2026-04-23 09:42)

### Nits
- [ ] `kanban-app/ui/src/test/spatial-nav-golden-path.test.tsx:438,1978` — fixture `AppWithGridAndRowSelectorEnterFixture` was not renamed when its inner helpers were (they became `RowWithSelectorInspectCommand` / `RowSelectorInspectInjector`). The outer name still says "Enter" even though it wires Space. Rename to `AppWithGridAndRowSelectorInspectFixture` for internal consistency with the renamed helpers.
- [ ] `kanban-app/ui/src/components/nav-bar.tsx:347` — command id `toolbar.inspect-board.activate` kept the `.activate` suffix while the command now fires on Space (an inspect verb, not an activation verb). Sibling ids `toolbar.board-selector.activate` and `toolbar.search.activate` are still activation verbs on Enter, so the mismatch stands out. Consider renaming to `toolbar.inspect-board.inspect` or similar to match the new vocabulary introduced elsewhere in the same change (e.g. `entity.activate.<m>` → `entity.inspect.<m>` in `entity-card.tsx`).
- [ ] `kanban-app/ui/src/lib/keybindings.test.ts:66-82` — test label is `"preserves modifiers on Space"` and the comment claims `"Mod+Space and Shift+Space should still be reachable"`, but only `Mod+Space` is asserted. Either add a `Shift+Space` assertion or drop the Shift claim from the comment. (Note: the current Shift-for-letters-only logic in `normalizeKeyEvent` means `Shift+Space` normalizes to just `"Space"` — documenting that explicitly in the test would be more valuable than the aspirational comment.)