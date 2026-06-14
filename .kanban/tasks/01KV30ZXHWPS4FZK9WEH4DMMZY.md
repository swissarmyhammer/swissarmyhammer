---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffaf80
project: builtin-commands
title: Surface "Edit Field" in the command palette and context menu (activate the field editor, same as Enter)
---
## Problem

`edit` is the ONE command that DOES make sense on a field (the complement of sibling task `2z8zavn`, which suppresses delete/archive/unarchive/inspect on fields). Picking "Edit Field" from the **command palette** or **context menu** should focus and activate that field's editor — the exact same outcome as pressing **Enter** on a focused field.

The behavior already exists but is **keybinding-only**:
- The command `field.edit` is defined in `builtin/plugins/app-shell-commands/commands/ui.ts` (`UI_SURFACE_COMMANDS`, ~L122–127) as `{ id: "field.edit", name: "Edit Field", scope: "ui:field", keys: { vim: "i", cua: "Enter" } }`. The `UiSurfaceCommandSpec` interface (~L55–59) carries only `id/name/scope/keys` — **no `context_menu`, no menu placement**, so it never appears in the palette's context-menu surface (the L111 comment notes "None of the four had a menu placement").
- The live activation behavior is registered on the webview command bus in `apps/kanban-app/ui/src/components/fields/field.tsx` (`editHandlers`, ~L597–646): a single `editClosure` keyed `"field.edit"` / `"field.editEnter"`, focus-gated to the field's `<FocusScope>` subtree. It drills into pills first, else calls `onEdit?.()` → `setEditingField(...)` (entity-card.tsx) → edit mode. This is the canonical "Enter on a focused field" path.

Context menus are already produced generically by `apps/kanban-app/ui/src/components/focus-scope.tsx` from `list command` results carrying `context_menu: true`, so a field moniker (`field:{type}:{id}.{name}`) already has a context-menu surface — `field.edit` just isn't in it.

## What

Surface `field.edit` on the palette + context menu and make a dispatch from those surfaces activate the **target** field's editor, reusing the existing `editClosure`.

- `builtin/plugins/app-shell-commands/commands/ui.ts`:
  - Promote `field.edit` out of the minimal `UI_SURFACE_COMMANDS` table (or extend `UiSurfaceCommandSpec` + the L539–551 registration `.map`) so it can declare `context_menu: true` with a sensible `context_menu_group` / `context_menu_order` (e.g. an "Edit" entry above the entity Cut/Copy/Paste group). Keep its existing `scope: "ui:field"` and `keys`. Model the richer registration on `app.inspect` (~L296–314), which already carries `context_menu` + `params: [{ name: "moniker", from: "target" }]`.
  - Keep `field.editEnter` (vim Enter parity) unchanged — keybinding-only.
  - Gating: rely on the `scope: "ui:field"` marker for palette visibility (the marker is in the focused chain whenever a field surface is focused). Do NOT blanket-add `applies_to: ["field"]` without checking the interplay: a `field:` SCOPE-CHAIN moniker resolves through `caption::focused_entity_type` to its CONTAINING entity (e.g. `task`), while a `field:` explicit context-menu `target` resolves to `"field"` — so an `applies_to` gate behaves differently for palette (focused) vs context-menu (targeted). Verify both surfaces with the list tests below before committing to a gate.
- `apps/kanban-app/ui/src/components/fields/field.tsx`:
  - Ensure a palette/context-menu dispatch reaches the correct field's `editClosure`. The keyboard/palette case (the field is already spatially focused) works as-is. For a context-menu dispatch carrying an explicit `field:` target that is NOT the spatially-focused field, the closure must operate on the targeted field — focus the target field first (dispatch `nav.focus` to the `field:` moniker, reusing `dispatchNavFocus`) then run the existing drill-in / `onEdit` fall-through, so a single code path serves Enter, palette, and context-menu.

Sibling context: pairs with `2z8zavn` (suppress the nonsensical field commands). Both touch `ui.ts`; they are independent concerns but coordinate on the field command surface.

## Acceptance Criteria
- [ ] With a field focused, the command palette lists "Edit Field"; invoking it puts that field into edit mode (editor mounted + DOM-focused), identical to pressing Enter.
- [ ] Right-clicking / opening the context menu on a field row shows "Edit Field"; invoking it puts THAT field (the menu's target) into edit mode, even if it was not the previously-focused field.
- [ ] A field with pills still drills into the first pill on `field.edit` (the existing two-outcome behavior is preserved for the new surfaces, not just the keybinding).
- [ ] `field.edit` does NOT surface on non-field focuses (no "Edit Field" on a task/tag/column/board entity palette or context menu).
- [ ] `field.editEnter` and the vim `i` / cua `Enter` keybindings are unchanged.

## Tests
- [ ] Browser test in `apps/kanban-app/ui/src/components/fields/` (extend or mirror `field.enter-edit.browser.test.tsx`): dispatch `field.edit` as the palette/context-menu surface would (not via the keymap) at a non-pill text field and assert the editor enters edit mode (the same assertion the Enter test makes); add a pill-field case asserting drill-in instead of edit.
- [ ] Context-menu target case: a browser/spatial test that dispatches `field.edit` with an explicit `field:` target that is not the currently-focused field and asserts the TARGET field enters edit mode (focus moved + editor mounted).
- [ ] Command-surface test: extend `apps/kanban-app/ui/src/test/ui-surface-plugin-commands-mirror.spatial.node.test.ts` (and/or the Rust `crates/swissarmyhammer-command-service/tests/list_applies_to.rs`) to assert `field.edit` carries `context_menu: true` and is offered when a field is focused/targeted but NOT on a non-field entity focus.
- [ ] `cd apps/kanban-app/ui && npm test -- field` (or the repo's vitest invocation) passes; the new field-surface assertions fail before the metadata + dispatch change and pass after.

## Workflow
- Use `/tdd` — write the failing palette/context-menu dispatch tests first (field enters edit mode from the non-keymap surface), then add the `context_menu` metadata + target-resolution and make them pass. #commands #field #command-driven-ui