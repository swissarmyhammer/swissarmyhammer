---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffdf80
project: spatial-nav
title: 'Unified global nav: delete inspector.move* shadows, route nav keys through one global keymap; preserve editor capture in edit mode'
---
## What

Inspector field Up/Down does nothing in production. Root cause: `inspector-focus-bridge.tsx:50-120` registers six keymap entries (`inspector.moveUp`, `inspector.moveDown`, `inspector.moveToFirst`, `inspector.moveToLast`, `inspector.nextField` (Tab), `inspector.prevField` (Shift+Tab)) that shadow the global `nav.*` commands and route to `broadcastNavCommand(...)` — a no-op stub at `entity-focus-context.tsx:359` that always returns `false`. The kernel never sees the keypress. The architectural fix in `01KQAW97R9XTCNR1PJAWYSKBC7` (eliminate `Option<Moniker>`) doesn't help here because the nav request never reaches the kernel.

The previous regression-guard tests in `01KQAXS8QKWCKFK8ENEMN7WHR1` pinned the kernel cascade and a hypothesized registration shape, but missed the actual production seam (the keymap shadow). They pass while production is broken.

## Design

Per the user's call:

> "the 'nav' key mapping should be global on the app — there is no need for 'inspector' or any other variant of nav when the whole idea is unified nav, and that also leads to duplicate code. The exception is — when I'm in an active CM6 or editor — navigation should be captured by that focused, active editor until we escape out of normal mode or escape out for cua/emacs back up to the field zone."

> "Zone > Scope. If we are a display, not captured by an editor, Up/Down needs to bubble up from the scope to the zone if there is no other sibling in the zone. When we are editing, the scope will have toggled to show an editor which should capture — and Esc handling in the field controls whether we are Display or Editor."

Translation:

1. **Single global nav keymap.** All `nav.up/down/left/right/first/last` commands live exclusively in `app-shell.tsx`'s `NAV_COMMAND_SPEC`. They thread `useSpatialFocusActions().navigate` with the matching `Direction`. The kernel cascade (iter 0: same-kind siblings → iter 1: escalate to parent zone → drill-out) handles "bubble up from scope to zone" naturally — no per-mode keymap needed.

2. **No per-context nav variants.** Delete `inspector.moveUp/Down/ToFirst/ToLast/nextField/prevField` from `inspector-focus-bridge.tsx`. Their key bindings are already covered by global `nav.up/down/first/last` (via `app-shell.tsx`) and global Tab→`nav.right` / Shift+Tab→`nav.left` (already in `BINDING_TABLES.cua` at `keybindings.ts:53-54`).

3. **Editor capture (display vs edit toggle).** When a field's editor is mounted (CM6 `.cm-editor` or native `<input>`/`<textarea>`/`<select>`/`[contenteditable]`), the global keybinding handler already defers via `isEditableTarget` at `keybindings.ts:279-296`. That means inside an active editor, native key handling owns Up/Down/Left/Right (cursor movement, vim-mode movement, etc.) and the kernel never sees them. This is correct behavior — confirm the deferral is intact after the keymap delete.

4. **Escape semantics unchanged.** Escape on a focused-but-not-editing field zone falls through to `app.dismiss` (close panel) via existing global handlers. Escape inside an editor exits edit mode via the field's `onCancel` chain (`field.tsx` → `inspector.exitEdit` or per-editor handler), restoring focus to the field zone, after which the next nav key is handled by the global handler.

5. **Keep edit-mode commands.** `inspector.edit` (vim:`i`, cua:`Enter`), `inspector.editEnter` (vim:`Enter`), and `inspector.exitEdit` are not nav — they toggle Display ↔ Editor for the focused field. They stay scoped to the inspector. (Future: even these could move to a `field.*` global scope, but that's outside the present fix.)

## What changes

### `kanban-app/ui/src/components/inspector-focus-bridge.tsx`

Delete six commands; keep the three edit-mode ones:

```tsx
const commands = useMemo<CommandDef[]>(
  () => [
    { id: "inspector.edit",      keys: { vim: "i",     cua: "Enter" }, execute: () => navRef.current?.enterEdit() },
    { id: "inspector.editEnter", keys: { vim: "Enter" },               execute: () => navRef.current?.enterEdit() },
    { id: "inspector.exitEdit",  /* no keys */                          execute: () => { if (navRef.current?.mode === "edit") navRef.current.exitEdit(); } },
  ],
  [],
);
```

Remove `broadcastNavCommand` import and the dead `broadcastRef`. Update the doc comment to reflect the new contract.

### `kanban-app/ui/src/lib/entity-focus-context.tsx`

`broadcastNavCommand` is now used by no one but its tests (per the grep, board-view and grid-view also reference it). Confirm whether those callers still need it. If yes — leave the no-op stub in place for now (the lib stays the same; only the inspector stops calling it). If the references in `board-view.tsx`, `grid-view.tsx`, `grid-view.nav-is-eventdriven.test.tsx`, etc. are also dead after this change, that's a separate scope (file a follow-up; do not touch in this PR).

### Optional: replace bad regression tests

`01KQAXS8QKWCKFK8ENEMN7WHR1` left two regression tests that pin a hypothesized shape (panel zone with three sibling field zones in inspector layer) but don't match production (entity FocusScope sits between panel zone and field zones in production). Decision: leave them in place (they pin a kernel-level invariant that's still useful as a regression guard) but supersede them with end-to-end tests that drive real keyboard input and assert the user-visible result, per the test plan below.

## Acceptance Criteria

All asserted by automated tests below — no manual smoke step.

### Inspector field nav routes through global handler

- [ ] In a mounted inspector for a task with multiple editable fields, focusing the first field zone and dispatching `keydown { key: "ArrowDown" }` produces a focus change to the next field zone — `useFocusedScope()` reports the next field's moniker.
- [ ] Symmetric: ArrowUp from the second field lands on the first.
- [ ] Tab from a focused field → focus moves to the next field (via global Tab→`nav.right` then beam search; or Tab→`nav.down` if the global table is updated to that — verify which is correct given vertical inspector layout).
- [ ] Shift+Tab from a focused field → focus moves to the previous field.
- [ ] Home → focus moves to the first field; End → focus moves to the last field.

### Editor capture preserved

- [ ] Pressing Enter on a focused editable field zone enters edit mode (`inspector.edit` fires; `editing` flips to true; the editor input gains DOM focus).
- [ ] While in edit mode for a CM6 text field (e.g., `name`, `description`), pressing ArrowDown moves the CM6 cursor within the document; `useFocusedScope()` does NOT change. (Pins editor capture via `isEditableTarget` in `keybindings.ts`.)
- [ ] While in edit mode for a native `<input>` field (e.g., a number / scalar editor), pressing ArrowLeft / ArrowRight moves the input's caret; `useFocusedScope()` does NOT change.
- [ ] Pressing Escape on a focused-and-editing field exits edit mode AND restores spatial focus to the field zone. After Escape, the next ArrowDown navigates to the next field zone.
- [ ] Pressing Escape on a focused-but-not-editing field zone falls through to `app.dismiss` (existing layer-pop chain). No regression on inspector close.

### Keymap surface

- [ ] `inspector.moveUp`, `inspector.moveDown`, `inspector.moveToFirst`, `inspector.moveToLast`, `inspector.nextField`, `inspector.prevField` are gone from `inspector-focus-bridge.tsx`. The InspectorFocusBridge's `commands` array contains only `inspector.edit`, `inspector.editEnter`, `inspector.exitEdit`.
- [ ] No call to `broadcastNavCommand` survives in `inspector-focus-bridge.tsx`.
- [ ] The global `nav.*` commands continue to fire from inside an open inspector (verified by the routing test below).

## Tests

### Frontend — `kanban-app/ui/src/components/inspector-focus-bridge.unified-nav.browser.test.tsx` (new file)

Mounts the production provider stack with a per-test backend, opens an inspector for a task, and drives real `KeyboardEvent`s through the global keybinding handler.

- [ ] `arrow_down_in_inspector_field_navigates_to_next_field` — open inspector, focus field 1 via `spatial_focus`, dispatch `keydown { key: "ArrowDown" }` on `document.activeElement`, assert `useFocusedScope()` reports field 2's moniker after the event.
- [ ] `arrow_up_in_inspector_field_navigates_to_prev_field` — symmetric.
- [ ] `tab_in_inspector_field_navigates_forward` — assert Tab moves to the next field (using whichever global Tab binding is in effect).
- [ ] `shift_tab_in_inspector_field_navigates_back`.
- [ ] `home_in_inspector_field_navigates_to_first_field`.
- [ ] `end_in_inspector_field_navigates_to_last_field`.
- [ ] `arrow_down_in_active_cm6_editor_does_not_change_spatial_focus` — open inspector, focus the name field, press Enter to enter edit mode, dispatch ArrowDown on the CM6 root; assert `useFocusedScope()` is unchanged AND CM6's selection moved (assert via `view.state.selection.main.head` change or DOM selection inspection).
- [ ] `arrow_left_in_active_native_input_editor_does_not_change_spatial_focus` — same with a number/scalar editor.
- [ ] `escape_in_active_editor_exits_edit_mode_and_restores_field_focus` — focus a field, Enter to edit, Escape; assert `editing === false`, `useFocusedScope()` reports the field moniker.
- [ ] `escape_on_focused_field_zone_dispatches_app_dismiss` — focus a field zone (no edit mode), dispatch Escape, assert `app.dismiss` was dispatched (panel close intent). Pin the existing layer-pop chain.

Test command: `bun run test:browser inspector-focus-bridge.unified-nav.browser.test.tsx` — all ten pass.

### Frontend — update `kanban-app/ui/src/lib/keybindings.test.ts`

Existing test references to `inspector.moveDown` / `inspector.moveUp` (per grep) need updating. Walk every occurrence and replace with the new contract: those bindings no longer exist; the global `nav.down` / `nav.up` handle ArrowDown / ArrowUp inside an inspector.

### Frontend — supersede or update existing tests

- `inspectors-container.spatial-nav.test.tsx`, `entity-inspector.field-vertical-nav.browser.test.tsx`, `entity-inspector.field-up-down.diagnostic.browser.test.tsx` — re-read each to confirm they pass on the new code path. If any assert on the existence of `inspector.moveDown` etc., update.

Test command: `bun run test:browser` (full UI suite) — all pass after the migration.

## Workflow

- **Use `/tdd`.** Write the new browser tests first, watch them fail (currently `inspector.moveDown` shadows global `nav.down` → the no-op stub fires → focus does not move). Delete the six inspector commands. Watch the tests pass. Update any other tests that asserted on the deleted command IDs.
- One file change in production code (`inspector-focus-bridge.tsx`), one new test file, plus minor updates to existing tests.
- This task supersedes the regression-guard tests added in `01KQAXS8QKWCKFK8ENEMN7WHR1` for the user-visible nav behavior; those kernel-level regression tests stay in place as a separate guard.
- **Coordinated separately**: the focus-debug-overlay z-strategy bug (overlays for inspector-layer scopes/zones get covered by window-layer overlays because `z-50` is global, not layer-relative) is not in scope here. File as a follow-up.

## Review Findings (2026-04-28 07:59)

Implementation summary: the six `inspector.move*` shadow commands and the `broadcastNavCommand` plumbing are cleanly deleted from `inspector-focus-bridge.tsx`. The surviving three edit-mode commands carry an excellent doc comment explaining the unified-nav contract and editor-capture deferral. `BINDING_TABLES.cua`'s Tab/Shift+Tab comment in `keybindings.ts:45-58` is updated. `keybindings.test.ts` migrates every stale `inspector.moveDown` / `inspector.moveUp` reference to the surviving `inspector.edit` / `inspector.editEnter` examples. The new browser test file `inspector-focus-bridge.unified-nav.browser.test.tsx` mounts the production provider stack plus AppShell and drives real keyboard events through 10 cases. Test results: 10/10 in the new browser file pass, 74/74 in `keybindings.test.ts` pass, the broader inspector suite (3 files, 13 tests) passes, the kernel-level Rust integration test (`swissarmyhammer-focus/tests/inspector_field_nav.rs`, 5 tests) passes, and `tsc --noEmit` is clean.

### Warnings

- [x] `kanban-app/ui/src/components/inspector-focus-bridge.unified-nav.browser.test.tsx:747` — `escape_in_active_editor_exits_edit_mode_and_restores_field_focus` only asserts that the editor unmounted (the `.cm-content` is gone or `contenteditable` flipped to `false`). It does **not** assert the second half of its name — that spatial focus is restored to the field zone. The original acceptance criterion was explicit: "After Escape, the next ArrowDown navigates to the next field zone." That post-Escape ArrowDown follow-up is not exercised. Without it, a regression where Escape exits edit mode but loses field-zone focus (e.g., focus drops to `<body>`) would still pass this test. Suggested fix: after the `waitFor` that confirms the editor unmounted, dispatch one more `ArrowDown` on `document` and assert exactly one new `spatial_navigate` call fires with `direction === "down"` and `key === nameZone.key` — that pins both halves of the criterion in one assertion.

  **Resolution (2026-04-28):** Added the post-Escape ArrowDown probe at the end of the test. After confirming the editor unmounted, the test now clears the IPC mock and dispatches `keydown { key: "ArrowDown" }` on `document`; it then asserts exactly one new `spatial_navigate` call fires with `key === nameZone.key` and `direction === "down"`. Both halves of the test name are now pinned in one assertion: edit mode exited (no `isEditableTarget` deferral) AND spatial focus is on the field zone (the `nav.down` execute closure threaded the field zone's key through `spatial_navigate`).

### Nits

- [x] `kanban-app/ui/src/components/inspector-focus-bridge.unified-nav.browser.test.tsx:592-679` and `:681-741` — The two editor-capture tests (`arrow_down_in_active_cm6_editor_does_not_change_spatial_focus`, `arrow_left_in_active_native_input_editor_does_not_change_spatial_focus`) verify the negative half of the contract (no `spatial_navigate` IPC fires) but skip the positive half: that the CM6 cursor / input caret actually moved. The test file's preamble (`:31-33`) explicitly promised the assertion via "DOM selection inspection" or `view.state.selection.main.head`. With only the negative assertion, a hypothetical regression that broke both global nav AND editor key handling would still pass. The negative pin alone is the load-bearing check (if `spatial_navigate` doesn't fire, only the editor's own keymap could have handled the keystroke), so this is a defense-in-depth nit, not a contract gap. Suggested fix: probe `cmContent.ownerDocument.getSelection()` or capture the CM6 view via a `data-cm-view` ref and assert `view.state.selection.main.head` advanced after the keypress; for the native input, assert `input.selectionStart` decremented after ArrowLeft.

  **Resolution (2026-04-28):** Both tests now pin both halves of their contracts.
  - **CM6 test:** switched from the single-line `name` field to the multi-line `body` field so ArrowDown is unambiguously meaningful. Capture the live `EditorView` via `EditorView.findFromDOM`, set the cursor to head=0, dispatch ArrowDown on `.cm-content`, then assert `view.state.selection.main.head > 0`. Negative half (no `spatial_navigate`) is preserved.
  - **Native input test:** switched to `userEvent.keyboard("{ArrowLeft}")` (Playwright-driven real keyboard) so the browser's native caret movement actually fires. `<input type="number">` does not expose `selectionStart`/`selectionEnd` per the HTML spec, so the positive half is pinned via DOM-focus retention (`document.activeElement === input`) plus value-stability (`input.value === "12345"`). Combined with the unchanged negative assertion (no `spatial_navigate`), this proves the editor's own native handler took the key rather than the global nav keymap.
- [x] `kanban-app/ui/src/components/inspector-focus-bridge.tsx:57-58` and `kanban-app/ui/src/components/inspector-focus-bridge.unified-nav.browser.test.tsx:34` — Both call sites reference `keybindings.ts:279-296` for the `isEditableTarget` deferral. The current source has `isEditableTarget` defined at lines 285-302 and called at line 341. Line numbers in inline cross-references drift naturally, but the doc comment is load-bearing because it points readers at the editor-capture invariant. Suggested fix: replace numeric line ranges with a symbol-anchored phrase like "via `isEditableTarget` in `keybindings.ts` (called from `createKeyHandler` before binding lookup)" so future edits to `keybindings.ts` don't silently stale the references.

  **Resolution (2026-04-28):** All three numeric line references replaced with the symbol-anchored phrase "via `isEditableTarget` in `keybindings.ts` (called from `createKeyHandler` before binding lookup)" — once in `inspector-focus-bridge.tsx`'s file-header doc comment, once in the test file's preamble, and once in the in-test comment for `arrow_down_in_active_cm6_editor_does_not_change_spatial_focus`.
