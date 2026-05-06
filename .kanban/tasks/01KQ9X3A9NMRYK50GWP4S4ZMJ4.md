---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffd980
project: spatial-nav
title: Enter must drill in, not inspect — restore the global nav.drillIn semantics across board zones and add field-edit on Enter
---
## What

Pressing **Enter** on a focused entity in the board currently dispatches `ui.inspect` instead of drilling in. The intended global semantics are:

> **Enter drills in.** For a `<FocusZone>`, drill in focuses the zone's first (or remembered) child. For a `<FocusZone moniker="field:…">`, drill in enters the field's edit mode. For a leaf with no editable affordance, Enter is a no-op (it does **not** open the inspector).

Inspect is reachable via Space (CUA), double-click, the context menu, and the command palette — none of which change in this task.

## Why this is broken today

Three forces collide on Enter:

1. **Global `nav.drillIn`** in `kanban-app/ui/src/components/app-shell.tsx:336` declares `keys: { vim: "Enter", cua: "Enter" }` and calls `actions.drillIn(focusedKey) → setFocus(moniker)`. This is the intended owner of Enter.

2. **`board.inspect`** in `kanban-app/ui/src/components/board-view.tsx:599` (`makeInspectCommand`) declares `keys: { vim: "Enter", cua: "Space" }` and is registered at the BoardView's `CommandScopeProvider`. Because the board scope is closer than the root scope in `extractScopeBindings`, `board.inspect` **shadows** `nav.drillIn` for vim Enter on every focused entity inside the board. That is the active bug.

3. **`<Field>`** in `kanban-app/ui/src/components/fields/field.tsx:363` is a `<FocusZone moniker="field:…">` with an `onEdit` callback ("user wants to enter edit mode (click, Enter)" per the prop docstring), but **no Enter binding** ever calls `onEdit`. Click-to-edit works via the inner `<div onClick={props.onEdit}>` in `FieldDisplayContent`; Enter has no path to `onEdit` today. Drill-in falls through to the kernel, which calls `child_entries_of_zone` — fields' display content is not registered as a spatial child, so the kernel returns `None`, the closure no-ops, and the user sees nothing happen on Enter even when focused on a field row.

The grid view (`grid-view.tsx:343` `grid.editEnter` with `keys: { vim: "Enter" }` calling `gridRef.current.enterEdit()`) already does the right thing — it owns Enter at the grid scope and routes it to edit mode. The board side and the field zone are the two places that still need to align.

## Approach

Three localised changes, one PR. Each at its natural seam:

### 1. Stop `board.inspect` from claiming Enter

`kanban-app/ui/src/components/board-view.tsx:603` — drop `vim: "Enter"` from `board.inspect.keys`. The board's "inspect focused entity" command keeps `cua: "Space"` (and any equivalent emacs binding the file already has). Inspect remains reachable via Space, double-click (`<Inspectable>`), context menu, and the command palette. The vim-Enter slot is restored to `nav.drillIn`.

### 2. Make `<Field>` zones own Enter as "enter edit mode"

`kanban-app/ui/src/components/fields/field.tsx` — when the field is **not already editing** and `onEdit` is defined, the field's `<FocusZone>` registers a per-zone command:

```ts
{
  id: "field.edit",
  name: "Edit Field",
  keys: { vim: "Enter", cua: "Enter" },
  execute: () => onEdit?.(),
}
```

Pass it through the `<FocusZone commands={...}>` prop so it lands in the field zone's `CommandScope`. The field-zone scope is closer than the root, so its `Enter` binding shadows the global `nav.drillIn` only when the focused thing is a field zone — exactly the precision-targeting `extractScopeBindings` is built for.

When `editing` is true, the editor element holds DOM focus and owns Enter via its own keymap (commit/newline depending on field type) — the field's own scope-level `field.edit` command must NOT register in edit mode, or it would dispatch back into edit mode while the user is already typing.

This stays a Field concern (no leaks to BoardView, no leaks to the field-edit infrastructure outside `<Field>`).

### 3. Verify `nav.drillIn` works for the existing zone consumers

No code change expected here, but the test suite must pin it:

- **Column zone** focused → Enter drills into the column's first or remembered card.
- **Inspector panel zone** focused → Enter drills into the panel's first child (typically the first inspector field row).
- **NavBar / perspective bar zones** focused → Enter drills into the zone's first leaf.

If any of those today returns the wrong thing, that is a kernel or registration bug to fix in this PR. (`drill_in` semantics are pinned by `swissarmyhammer-focus/tests/drill.rs`; if a regression in registration leaves a zone with no spatial children, the test below will catch it.)

## Acceptance Criteria

All asserted by automated tests below — no manual smoke step.

- [x] **vim Enter on a focused card does NOT dispatch `ui.inspect`.** It drills in via `nav.drillIn`. (For a leaf-shaped card today, drill_in returns `None` and Enter is a no-op — the regression-guard test asserts zero `ui.inspect` calls. The card-becomes-zone work is tracked separately by `01KQ5PEHWTEVTKPS2JHSZTXNBE`.)
- [x] **vim Enter on a focused column drills into the column's first card.** `useFocusedScope()` reports the moniker of the first card (or the column's `last_focused`).
- [x] **vim Enter on a focused inspector panel zone drills into the panel's first inspector field zone.**
- [x] **Enter on a focused field zone (display mode, editable field) puts the field into edit mode.** The `Field`'s `editing` prop flips to `true`, the `FieldEditor` renders, and DOM focus lands on the editor input. The corresponding browser-level assertion: after pressing Enter, the focused element is an `<input>` / `<textarea>` / contenteditable inside the field zone.
- [x] **Enter on a focused field zone (display mode, non-editable field — `resolveEditor(fieldDef) === "none"`) is a no-op.** The field stays in display mode; no `ui.inspect` is dispatched.
- [x] **Enter on a focused field zone (already editing) does NOT re-trigger `field.edit`.** The editor input owns Enter (commit / newline per its own keymap). Asserted by spying on `onEdit` and confirming it is NOT called when `editing={true}` and Enter is fired.
- [x] **Space on a focused card still dispatches `ui.inspect`** (CUA path unchanged). Regression guard for the kept binding.
- [x] **Double-click on a card still dispatches `ui.inspect`** (regression guard for `<Inspectable>`). (Pinned by the existing `inspectable.space.browser.test.tsx#dblclick_on_inspectable_still_dispatches_inspect`, which still passes after this change.)
- [x] **The Enter binding on `ui.entity.startRename` (cua/vim/emacs) keeps working for the active perspective tab.** Regression guard — `ui.entity.startRename` is scope-pinned to `entity:perspective` so it should already be unaffected, but confirm the binding still fires.

## Tests

All tests are automated. No manual verification.

### Frontend — `kanban-app/ui/src/components/board-view.enter-drill-in.browser.test.tsx` (new file)

Mounts the production provider stack against the per-test backend the existing browser tests already use. Asserts on observable focus state and dispatch records (no mocked dispatcher pinned by id; allow real dispatchers, capture every `invoke` call).

- [x] `enter_on_focused_card_does_not_dispatch_inspect_in_vim` — keymap vim, focus a card via click or `setFocus`, fire `keydown { key: "Enter" }`, await one tick, assert dispatch records contain zero `ui.inspect` invocations.
- [x] `enter_on_focused_card_does_not_dispatch_inspect_in_cua` — same as above with keymap CUA. (Regression guard — Enter has never been bound to inspect in CUA, but pin it.)
- [x] `space_on_focused_card_still_dispatches_inspect_in_cua` — keymap CUA, focus a card, fire `keydown { key: " " }`, assert exactly one `ui.inspect` dispatch with `target = task:<id>`.
- [x] `enter_on_focused_column_drills_into_first_card` — focus a column zone, fire Enter, assert `useFocusedScope()` reports the first card's moniker.
- [x] `enter_on_focused_column_with_remembered_focus_drills_into_remembered_card` — focus a column, focus a non-first card, drill out to the column, fire Enter, assert focus lands on the remembered card.

Test command: `bun run test:browser board-view.enter-drill-in.browser.test.tsx` — all five pass.

### Frontend — `kanban-app/ui/src/components/fields/field.enter-edit.browser.test.tsx` (new file)

Mounts a `<Field>` inside the production provider stack with realistic `<FocusLayer>` + `<FocusZone>` ancestry.

- [x] `enter_on_field_zone_in_display_mode_enters_edit_mode` — render an editable field (e.g. text), focus the field zone, fire Enter, assert the field's `editing` flipped to `true` (observable via DOM: the editor input is now in the document).
- [x] `enter_on_field_zone_in_display_mode_focuses_the_editor_input` — same as above, after Enter assert `document.activeElement` is the editor input.
- [x] `enter_on_field_zone_already_in_edit_mode_does_not_call_onEdit_again` — start with `editing={true}`, spy on `onEdit`, fire Enter on the editor element, assert `onEdit` is NOT called (the editor's keymap owns Enter).
- [x] `enter_on_non_editable_field_zone_is_noop` — render a field whose `fieldDef.editor === "none"` (or use a real read-only field def); focus the field zone, fire Enter, assert the field stays in display mode and no `ui.inspect` is dispatched.

Test command: `bun run test:browser fields/field.enter-edit.browser.test.tsx` — all four pass.

### Frontend — `kanban-app/ui/src/components/inspectors-container.enter-drill-in.browser.test.tsx` (new file)

- [x] `enter_on_focused_panel_zone_drills_into_first_field` — open inspector for a task, focus the panel zone, fire Enter, assert focus lands on the panel's first inspector field zone (`field:task:<id>.<name>`).

Test command: `bun run test:browser inspectors-container.enter-drill-in.browser.test.tsx` — passes.

### Frontend — augment `kanban-app/ui/src/components/perspective-tab-bar.enter-rename.spatial.test.tsx`

- [x] Add a regression test: with the active perspective tab focused, vim Enter still dispatches `ui.entity.startRename` (scope-pinned binding survives). Existing tests already cover this — verify it still passes after the changes, plus the new explicit regression guard `regression: scope-pinned ui.entity.startRename: vim Enter still fires after Enter-drill-in card`.

Test command: `bun run test:browser perspective-tab-bar.enter-rename.spatial.test.tsx` — all pre-existing tests plus the regression guard pass.

## Workflow

- Use `/tdd` — write the failing tests first, then make the three changes.
- Single ticket, single concern (Enter ownership). Do not split this into "one card per zone type". Every variant exercises the same chain — the global `nav.drillIn` binding plus precision-targeted scope-level overrides on field zones (and the existing grid case, which doesn't change).

## Verification Results

- `pnpm vitest run`: **1814 passed | 1 skipped (1815)** — up from 1797/1798 baseline (+17 net).
- `pnpm tsc --noEmit`: clean (0 errors, 0 warnings).
- `cargo build --workspace`: clean.

## Implementation Notes

- `board.inspect` was already removed prior to this card (by `01KQ9XJ4XGKVW24EZSQCA6K3E2`); `board-view.tsx` is unchanged in this PR (the static guard `board-view.space-inspect.guard.node.test.ts` continues to enforce this).
- The actual code change is a single-file edit to `kanban-app/ui/src/components/fields/field.tsx`: register a per-zone `field.edit: Enter` command via `<FocusZone commands={…}>` when `editing === false && onEdit !== undefined`.
- All other deliverables are tests:
  - `kanban-app/ui/src/components/board-view.enter-drill-in.browser.test.tsx` (new, 5 tests)
  - `kanban-app/ui/src/components/fields/field.enter-edit.browser.test.tsx` (new, 4 tests)
  - `kanban-app/ui/src/components/inspectors-container.enter-drill-in.browser.test.tsx` (new, 1 test)
  - `kanban-app/ui/src/components/perspective-tab-bar.enter-rename.spatial.test.tsx` (augmented with 1 regression test)

## Review Findings (2026-04-27 08:25)

Verified the six reviewer-checklist points from the user's request:

1. `field.edit` registration is correctly conditional — `useMemo` returns `EMPTY_COMMANDS` when `editing || !onEdit`, otherwise registers the command. Both branches confirmed.
2. Scope-shadowing is precise — `extractScopeBindings` walks inside-out with first-key-wins; field zone's `field.edit Enter` wins over global `nav.drillIn Enter` only when a field zone holds spatial focus. Confirmed against `keybindings.ts` lines 386-392.
3. Five `board-view.enter-drill-in.browser.test.tsx` tests each drive vim/cua Enter on focused cards/columns and assert zero `ui.inspect` invocations. Test #5 includes belt-and-suspenders DOM check on `data-focused`.
4. Four `field.enter-edit.browser.test.tsx` tests cover all AC branches: editable→Enter→edit, focus→editor, already-editing→noop on `onEdit`, non-editable→noop and zero `ui.inspect` dispatch.
5. `inspectors-container.enter-drill-in.browser.test.tsx` pins panel zone → drill_in IPC → `ui.setFocus` fanout for the resolved field moniker.
6. `perspective-tab-bar.enter-rename.spatial.test.tsx` regression guard confirms vim Enter on the active perspective tab still mounts the rename editor and dispatches no `ui.inspect`.

Vitest re-run: 1814 passed | 1 skipped (1815) — matches the implementer's claim.

Cargo build was not re-run (out of scope for the narrow change; pre-existing flaky cargo tests are out of scope per the user's note).

### Warnings
- [x] `kanban-app/ui/src/components/inspectors-container.enter-drill-in.browser.test.tsx:63` — Resolved (2026-04-27 08:32). Deleted the unused `emitUiStateChanged` helper. The surrounding `MutableUIState` / `backendState` / `uiStateSnapshot` plumbing is genuinely consumed by the `get_ui_state` IPC mock (and reset in `beforeEach`), so it is left in place — only the truly dead helper was removed.

## Trivial Sweep (2026-04-27 08:32)

Authorized scope expansion: removed unused `waitFor` import at `kanban-app/ui/src/components/column-view.scroll-rects.browser.test.tsx:39`. This was a leftover from card `01KQ9XBAG5P9W3JREQYNGAYM8Y` blocking `tsc --noEmit` for every card on the branch. One-line fix.

## Final Verification (2026-04-27 08:32)

- `pnpm tsc --noEmit`: clean (0 errors, 0 warnings).
- `pnpm vitest run`: 1814 passed | 1 skipped (1815) — unchanged.
- `cargo build --workspace`: clean.