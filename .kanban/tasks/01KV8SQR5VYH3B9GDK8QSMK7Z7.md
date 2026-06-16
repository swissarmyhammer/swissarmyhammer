---
position_column: todo
position_ordinal: fd80
project: ui-command-cleanup
title: 'Perspective ("view") tabs: no "Switch to <perspective>" in context menu or palette — mirror the view-switch fix'
---
## What

The perspective tab-bar buttons (the "view buttons" at the top of the board that switch which saved view you're looking at) have **no right-click context menu to switch to a perspective**, and the per-perspective switch rows are **not findable as "Switch to …" in the command palette**.

This is the exact symmetric gap that was already closed for **views** but never applied to **perspectives**. The fix is to make `emit_perspective_goto` mirror `emit_view_switch`.

### Background — the view path is the precedent (already done)

In `crates/swissarmyhammer-kanban/src/scope_commands.rs`:
- `emit_view_switch` (lines 312-344) emits one `view.set` row per view for the palette, and flips the row whose `view:{id}` moniker is in the right-click `scope_chain` to `context_menu: true` (card `01KV5K29FFQJTBER6HYA4J2DW6`). Caption: `"Switch to View «name»"` via `view_switch_caption` (lines 274-283).
- `emit_perspective_goto` (lines 446-477) emits one `perspective.switch` row per perspective but **hardcodes `context_menu: false` for every row** (line 468) with **no `scope_chain` parameter and no in-scope detection**, and uses the caption `"Go to Perspective: «name»"` (line 464) — so a user searching the palette for "Switch to" finds nothing.

The frontend is already fully wired (no frontend change needed for the context menu to work once the backend marks a row): the perspective tab mounts `<CommandScopeProvider moniker={moniker("perspective", perspective.id)}>` → `perspective:{id}` (`apps/kanban-app/ui/src/components/perspective-tab-bar.tsx:1107`) and the tab button has `onContextMenu={handleContextMenu}` (line 1608, `useContextMenu` at 1577). The right-click scope chain therefore already carries `perspective:{id}`; `context_menu_only` filtering in `commands_for_scope` (scope_commands.rs:669-671) drops the perspective rows today purely because they are all `context_menu: false`.

This aligns with the `ui-command-cleanup` model: the per-perspective switch commands stay **backend-defined** (in `emit_perspective_goto`); the UI only renders/dispatches them.

### Changes — all in `crates/swissarmyhammer-kanban/src/scope_commands.rs`

- [ ] Give `emit_perspective_goto` a `scope_chain: &[String]` parameter and compute `in_scope: HashSet<&str>` from monikers stripped of the `perspective:` prefix — copy the pattern at `emit_view_switch` lines 318-323.
- [ ] Set `context_menu: in_scope.contains(perspective.id.as_str())` instead of the hardcoded `false` (line 468), so right-clicking perspective X surfaces exactly its own "Switch to Perspective «X»" row and siblings stay palette-only — identical to the view design (lines 285-301 doc).
- [ ] Rename the caption from `"Go to Perspective: «name»"` to `"Switch to Perspective «name»"` for palette findability and symmetry with `view_switch_caption`. Add a `perspective_switch_caption(name)` helper mirroring `view_switch_caption` (lines 274-283), keeping the `BLANK_PERSPECTIVE_NAME_PLACEHOLDER` ("Untitled") behavior for blank names (lines 451-458).
- [ ] Update the caller `emit_dynamic_commands` (line 606) to pass `scope_chain` into `emit_perspective_goto`.
- [ ] Update the existing unit test `blank_perspective_names_get_the_untitled_placeholder_caption` (lines 1460-1487) for the new caption text, and update the module doc-comment at lines 31-36 / 570-586 which describes perspective rows as "Go to Perspective" / palette-only.

## Acceptance Criteria
- [ ] Right-clicking a perspective tab shows a "Switch to Perspective «name»" entry that dispatches `perspective.switch` with `args.perspective_id` for that tab, and switches the active perspective.
- [ ] Sibling perspectives are NOT in the right-click menu (only the in-scope tab's own switch row is `context_menu: true`), matching view behavior.
- [ ] The command palette lists "Switch to Perspective «name»" for every perspective (one `perspective.switch` row each, palette-findable under "switch").
- [ ] Blank-named perspectives render the "Untitled" placeholder in both surfaces.
- [ ] No regression: views still behave exactly as before; existing perspective/view/palette/context-menu tests stay green.

## Tests
- [ ] **Backend unit test** in `crates/swissarmyhammer-kanban/src/scope_commands.rs` `mod tests`: add a test (mirroring `blank_perspective_names_get_the_untitled_placeholder_caption`) that calls `emit_perspective_goto` with a `scope_chain` containing `perspective:01P2` and asserts (a) every perspective gets a row with the "Switch to Perspective …" caption, (b) only the `01P2` row has `context_menu == true`, (c) all others have `context_menu == false`. Add an assertion that with an empty `scope_chain` no row is `context_menu: true` (palette-only).
- [ ] **Frontend test** — extend `apps/kanban-app/ui/src/components/perspective-tab-bar.context-menu.test.tsx` (real `useContextMenu` → scope-chain → `list command`): seed `mockRegistry` with a `perspective.switch` row marked `context_menu: true` (and `args.perspective_id`), right-click a perspective tab, and assert the "Switch to Perspective «name»" item renders and dispatches `perspective.switch` with the correct `perspective_id`. This is the regression guard that fails before the backend fix (today no perspective row is ever `context_menu: true`).
- [ ] Run `cargo test -p swissarmyhammer-kanban scope_commands` — green.
- [ ] Run `cd apps/kanban-app/ui && npm test` (`tsc --noEmit && vitest run`) — both `unit` and `browser` projects green.

## Notes
- Terminology: the user's "view buttons" are the perspective tabs. True **views** (`view.set`, LeftNav `ViewButton`) already have this exact context-menu + palette behavior — this task brings perspectives to parity.
- Related but distinct: `^yrdj19h` (emit_view_switch double-definition). Not a dependency.

## Workflow
- Use `/tdd` — write the failing backend unit test (in-scope `context_menu: true`) and the failing frontend context-menu test first, watch them fail against the current hardcoded `context_menu: false`, then implement the `emit_perspective_goto` change to make them pass. #bug