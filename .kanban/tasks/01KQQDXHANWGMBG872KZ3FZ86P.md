---
assignees:
- claude-code
depends_on:
- 01KQPVRYW2CRCNSDR3XMSPRN3B
position_column: todo
position_ordinal: cb80
project: spatial-nav
title: nav.drillIn should focus an editable descendant when the focused leaf has no spatial children
---
## What

When spatial focus is on a leaf whose DOM root contains an editable surface (CM6 editor, `<input>`, `<textarea>`, `[contenteditable]`), pressing Enter should move DOM focus *into* that editable so the user can start typing. The user-facing case that prompted this is the perspective bar's filter formula leaf: pressing Enter should focus the CM6 filter editor.

The fix must be **generic** — implemented as a fall-through inside the existing `nav.drillIn` global command — not a per-component command (no `filter.focus`, no `filter.edit` analogue). Any leaf that wraps an editable surface should benefit automatically.

### Current state

`kanban-app/ui/src/components/app-shell.tsx::buildDrillCommands` (line 346) defines the global `nav.drillIn: Enter` command:

```ts
execute: async () => {
  const actions = refs.spatialActionsRef.current;
  const focusedFq = actions.focusedFq();
  if (focusedFq === null) return;
  const result = await actions.drillIn(focusedFq, focusedFq);
  // The kernel always returns an FQM. When `result === focusedFq`
  // the caller's setFocus call is idempotent (no descent happened),
  // which visually matches the legacy "null → no-op" behavior.
  refs.setFocusRef.current(result);
},
```

The "no descent" branch (`result === focusedFq`) is currently a visible no-op. The existing per-component pattern (`field.edit` in fields/field.tsx:545, `ui.entity.startRename` in perspective-tab-bar.tsx:392) registers scope-local commands that shadow `nav.drillIn` and call the editor's `focus()` handle directly. That works but requires every editor-bearing leaf to register its own command — exactly the "hard-coded" pattern the user wants to avoid.

### Fix shape

Extend the no-descent fall-through in `buildDrillCommands.execute` (app-shell.tsx:346): when the kernel returns the same FQM (no spatial descent), find the focused leaf's DOM element via `document.querySelector(\`[data-moniker="${focusedFq}"]\`)` and look for the first editable descendant. Call `.focus()` on it.

A small helper in `kanban-app/ui/src/lib/keybindings.ts` already encodes "what counts as editable" — see `isEditableTarget` (line 285): `INPUT`, `TEXTAREA`, `SELECT`, `.cm-editor`, `[contenteditable]`. Reuse the same selector list (extract a tiny `EDITABLE_SELECTOR` constant) for symmetry — same definition, two consumers.

Add a sibling helper `findEditableDescendant(host: Element): HTMLElement | null` (in `kanban-app/ui/src/lib/keybindings.ts` next to `isEditableTarget`, exported) that runs `host.querySelector(EDITABLE_SELECTOR)` and returns the first match if it is `HTMLElement`. The drill-in fall-through calls that helper, and on a hit calls `.focus()` and stops — no `setFocus` IPC needed (the inner editor already owns DOM focus, and the keybindings handler's `isEditableTarget` short-circuit (line 341) will route subsequent keystrokes to the editor's local keymap).

### Why this is generic

Once landed, every leaf that wraps a CM6 editor / `<input>` / `<textarea>` / `[contenteditable]` automatically supports Enter-to-edit. No new command, no hard-coded segment list, no per-leaf wiring. The filter editor leaf benefits because its `<FocusScope>` wrapper (per task `01KQPVRYW2CRCNSDR3XMSPRN3B`) emits `[data-moniker]` on a `<div>` whose descendant is the CM6 `.cm-editor`.

The existing per-component `field.edit` / `ui.entity.startRename` commands continue to win where they are registered (scope-local shadows global) — this task does NOT delete or alter them. Consolidating onto the generic mechanism is a separate cleanup task.

### Dependency

This task depends on **`01KQPVRYW2CRCNSDR3XMSPRN3B`** (Filter formula bar lacks a FocusScope) — without that wrapper, the filter editor is not a leaf in the spatial graph and Enter has nothing to drill into. Reference that task in `depends_on` when scheduling.

### Files to modify

- `kanban-app/ui/src/lib/keybindings.ts` — extract `EDITABLE_SELECTOR` from the `isEditableTarget` body; add and export `findEditableDescendant(host)`.
- `kanban-app/ui/src/components/app-shell.tsx::buildDrillCommands` — call `findEditableDescendant` in the no-descent branch and `.focus()` the result before falling through to the idempotent `setFocus`.

### Out of scope

- Removing per-component `field.edit` / `ui.entity.startRename` commands — separate refactor.
- Auto-drilling into non-editable focusable controls (buttons, links). The contract here is "Enter opens an editor", not "Enter clicks a button".
- Modifier Enter combos (Mod+Enter, Shift+Enter) — those keep their existing semantics.

## Acceptance Criteria

- [ ] With spatial focus on a leaf whose root carries `[data-moniker]` and contains a `.cm-editor` (or `<input>` / `<textarea>` / `[contenteditable]`) descendant, pressing Enter calls `.focus()` on that descendant — the editor receives DOM focus and subsequent keystrokes are routed to the editor's keymap (the existing `isEditableTarget` short-circuit in keybindings.ts:341 handles the routing).
- [ ] On the perspective bar filter formula leaf (`filter_editor:{perspectiveId}`), pressing Enter focuses the CM6 filter editor — the user can immediately type a filter expression. No `filter.*` scope command was added.
- [ ] On a leaf with no editable descendant and no spatial children, pressing Enter is still a visible no-op (idempotent `setFocus` of the focused FQM, same as today).
- [ ] No regression in leaves whose scope already registers a local Enter binding (`field.edit` on `<Field>`, `ui.entity.startRename` on perspective tabs, `view.activate` on left-nav buttons): the local binding still wins because scope-local commands shadow global.
- [ ] No double-fire: when the generic fall-through focuses an editable, no extra `spatial_focus` / `ui.setFocus` IPC is dispatched.

## Tests

- [ ] Unit test in `kanban-app/ui/src/lib/keybindings.test.ts`: `findEditableDescendant` returns the first editable descendant for each of `<input>`, `<textarea>`, `<select>`, `.cm-editor`, `[contenteditable]` hosts; returns `null` for a host with no editable descendants.
- [ ] Integration test in `kanban-app/ui/src/components/app-shell.test.tsx` (or a new `app-shell.drill-in-editable.spatial.test.tsx`): mount a minimal `<AppShell>` with a synthetic leaf whose `[data-moniker]` element wraps an `<input>`. Drive `focusedFq` to that leaf, fire `keydown Enter` through the global keymap path, and assert `document.activeElement === inputEl`. Mock the kernel to return the focused FQM unchanged so the no-descent branch fires.
- [ ] Regression test in `kanban-app/ui/src/components/perspective-bar.spatial.test.tsx` (depends on the wrapper from `01KQPVRYW2CRCNSDR3XMSPRN3B`): with focus on the `filter_editor:{perspectiveId}` leaf, fire `keydown Enter` and assert `document.activeElement` is inside `.cm-editor` (e.g. `[contenteditable=true]` matched within the editor host).
- [ ] Negative regression in the same file: with focus on a `perspective_tab:{id}` leaf (which already registers `ui.entity.startRename` for Enter), fire `keydown Enter` and assert `triggerStartRename` is invoked AND no editable descendant of the tab receives DOM focus — proving the local-shadow rule still wins.
- [ ] Run `pnpm -C kanban-app/ui test app-shell perspective-bar keybindings` and confirm the new tests pass.

## Workflow

- Schedule after `01KQPVRYW2CRCNSDR3XMSPRN3B` lands so the filter editor leaf exists.
- Use `/tdd` — write the `findEditableDescendant` unit test and the app-shell integration test (RED), extract `EDITABLE_SELECTOR` and add `findEditableDescendant`, then add the fall-through branch in `buildDrillCommands.execute` (GREEN). Verify the perspective-bar regression and the negative `perspective_tab` regression both pass.
