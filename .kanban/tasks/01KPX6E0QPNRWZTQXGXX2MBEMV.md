---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff9480
project: spatial-nav
title: Rebind ui.inspect from Enter to Space across the codebase
---
## What

`ui.inspect` is currently bound to Enter. Migrate it to Space so Enter can become the universal "activate / drill into the focused scope" verb, matching macOS Finder's Quick Look (Space) / Open (Enter) convention and every major vim-style file manager.

## Acceptance Criteria

- [x] `ui.inspect` in `swissarmyhammer-commands/builtin/commands/ui.yaml` has `keys: { vim: Space, cua: Space, emacs: Space }`
- [x] Pressing Space on a focused card opens the inspector for that card's entity
- [x] Pressing Space on a focused row selector opens the inspector
- [x] Pressing Space on a focused toolbar Inspect button opens the board inspector
- [x] Pressing Space on a focused column header opens the inspector for the column entity
- [x] Pressing Space on a focused scrollable element with an inspect target does NOT scroll the page
- [x] Pressing Enter on the above scopes NO LONGER opens the inspector
- [x] Grid cell + Enter still enters edit mode (unchanged)
- [x] Inspector field + Enter still enters edit mode (unchanged)
- [x] LeftNav button + Enter still switches view (unchanged)
- [x] Perspective tab + Enter still switches perspective (unchanged)
- [x] All existing npm and Rust tests green
- [x] New or updated tests assert the Space binding, not Enter, for inspect

## Tests

- [x] Update `kanban-app/ui/src/components/data-table.test.tsx` — row selector Space opens inspector
- [x] Update card and toolbar inspect tests similarly
- [x] Add a regression test: pressing Enter on a focused card does NOT dispatch `ui.inspect`
- [x] Add a browser-behavior test: pressing Space on a scrollable element resolves the inspect binding and does NOT scroll the page
- [x] Run `cd kanban-app/ui && npm test` — green
- [x] Run `cargo test -p swissarmyhammer-commands` — YAML parsing tests still green

## Review Findings (2026-04-23 09:42) — all addressed

### Nits (all fixed)

- [x] `spatial-nav-golden-path.test.tsx` fixture `AppWithGridAndRowSelectorEnterFixture` → renamed to `AppWithGridAndRowSelectorInspectFixture` (both declaration at line 438 and usage at line 1978).

- [x] `nav-bar.tsx:347` command id `toolbar.inspect-board.activate` → renamed to `toolbar.inspect-board.inspect` to match the new Enter=activate / Space=inspect vocabulary (sibling ids `.board-selector.activate` and `.search.activate` stay as activation verbs on Enter).

- [x] `keybindings.test.ts:66-82` — "preserves modifiers on Space" test rewritten to explicitly document that Shift+Space normalizes to plain `"Space"` (because the Shift-prefix logic only applies to printable letters, not named keys). Test now asserts both `Mod+Space → "Mod+Space"` AND `Shift+Space → "Space"`, with a comment explaining the asymmetry.
