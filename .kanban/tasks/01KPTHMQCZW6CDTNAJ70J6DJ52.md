---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffff8480
title: 'Left-nav view buttons: add context menu with "Switch to <view>" — mirror perspective-tab pattern'
---
## What

Right-click on a view button in the left-nav sidebar currently falls through to the OS default menu — there is no per-view context menu. The user-facing requirement: show the same `Switch to <ViewName>` entry that the command palette offers, scoped to the specific view that was right-clicked. Not hardcoded — wire it through the existing command-scope + context-menu pipeline the same way `perspective-tab-bar.tsx` does for perspective tabs.

**Current state (verified by code trace)**:
- `kanban-app/ui/src/components/left-nav.tsx:28-62` — `LeftNav` renders a `<button>` per view with `onClick={() => dispatch('view.switch:${view.id}')}` but **no `onContextMenu`** and **no per-view `CommandScopeProvider`** wrapper.
- `swissarmyhammer-kanban/src/scope_commands.rs:238-259` — `emit_view_switch` generates one `ResolvedCommand` per view with `context_menu: false`. Block comment explains the current stance: "view switching is a navigation action that belongs in the palette, not on right-click." This task explicitly reverses that stance, but only when the specific view is in scope.
- `commands_for_scope` at `scope_commands.rs:491-535` already filters by the `context_menu_only` flag (line 529-531). The backend is the single source of truth for what appears in right-click.
- Working pattern to mirror: `kanban-app/ui/src/components/perspective-tab-bar.tsx:267-294` wraps each tab in `<CommandScopeProvider moniker={moniker("perspective", p.id)}>`, then the inner `PerspectiveTab` (lines 367-417) calls `const handleContextMenu = useContextMenu()` (from `kanban-app/ui/src/lib/context-menu.ts`) and attaches it as `onContextMenu` on the tab button.
- `moniker("view", view.id)` → `"view:<id>"` matches the format the backend expects in `scope_chain`.

**Why this wiring is correct**: `useContextMenu` (lib/context-menu.ts) reads `CommandScopeContext` → passes `scopeChain` and `contextMenu: true` to `invoke("list_commands_for_scope", ...)`. Backend returns commands whose `context_menu` flag is `true`. So making `view.switch:{id}` `context_menu: true` when-and-only-when its moniker is in the chain produces exactly "Switch to <this view>" on right-click.

## Approach

1. **Backend** — `swissarmyhammer-kanban/src/scope_commands.rs`:
   - Modify `emit_view_switch` (lines 238-259) to accept `scope_chain: &[String]` and set `context_menu: true` **only** for the view whose id is present as `view:{id}` in the chain. All other views keep `context_menu: false`.
     - The in-scope check: `scope_chain.iter().any(|m| m == &format!("view:{}", view.id))`.
     - Mirrors the scope-chain-filtering pattern from `emit_entity_add` (lines 404-439).
   - Update the call site in `emit_dynamic_commands` (line 471) to pass the `scope_chain` argument through.
   - Update the block comment at lines 233-237 to describe the new per-view `context_menu` behavior.
   - Palette behavior (`context_menu_only=false`) is unchanged — all views still appear.

2. **Frontend** — `kanban-app/ui/src/components/left-nav.tsx`:
   - Extract a `ScopedViewButton` sub-component that mirrors `ScopedPerspectiveTab` from perspective-tab-bar.tsx:267-294. It wraps the `<button>` in `<CommandScopeProvider moniker={moniker("view", view.id)}>`.
   - Inside `ScopedViewButton`, call `const handleContextMenu = useContextMenu();` and attach `onContextMenu={handleContextMenu}` to the `<button>`.
   - Add imports: `useContextMenu` from `@/lib/context-menu`, `CommandScopeProvider` from `@/lib/command-scope`, `moniker` from `@/lib/moniker`.
   - Map over `views` and render `<ScopedViewButton key={view.id} view={view} isActive={...} />`.

3. Do **not** add new commands (rename/delete/duplicate) — per clarification, scope is strictly "Switch to <view>" only. Future work if/when those commands exist.

## Acceptance Criteria

- [x] Right-clicking a view button in the left-nav opens a native context menu with exactly one item: `Switch to <ViewName>` for that specific view.
- [x] Selecting that item dispatches `view.switch:{viewId}` and the board switches to that view.
- [x] Right-clicking view A's button does NOT show "Switch to B" / "Switch to C" entries.
- [x] The command palette (Cmd-K / Ctrl-K) still shows ALL `Switch to <view>` entries — palette behavior unchanged.
- [x] Left-click on the view button still dispatches `view.switch:{viewId}` (no regression).
- [x] The scope moniker attached to the button is `view:{id}` (same shape `useContextMenu` reads via `CommandScopeContext`).

## Tests

- [x] New Rust test `view_switch_context_menu_only_emits_in_scope_view` in `swissarmyhammer-kanban/src/scope_commands.rs` tests module.
- [x] New Rust test `view_switch_palette_still_emits_all_views` in the same test module.
- [x] New browser test `kanban-app/ui/src/components/left-nav.browser.test.tsx` covers right-click scope chain, show_context_menu payload, and left-click regression.
- [x] Existing tests still pass: scope_commands view_switch tests (5/5), views-container.test.tsx, perspective-tab-bar.test.tsx (28/28).
- [x] Run: `cargo test -p swissarmyhammer-kanban --lib scope_commands::tests::view` → 5 passed. `npx vitest run --project browser left-nav views-container perspective-tab-bar` → 35 passed.

## Workflow

- Use `/tdd`. Write the failing Rust test `view_switch_context_menu_only_emits_in_scope_view` first, make it pass by modifying `emit_view_switch` and its call site. Then write the failing `left-nav.browser.test.tsx`, make it pass by adding `ScopedViewButton` + `useContextMenu` wiring.
- Do not modify `useContextMenu`, `commands_for_scope`'s top-level flow, or any command other than `emit_view_switch`. Scope is tight by design. #ux #commands #frontend

## Implementation Notes

- Backend `emit_view_switch` now takes `scope_chain: &[String]`; the in-scope check is `scope_chain.iter().any(|m| m == &format!("view:{}", view.id))`. This mirrors `emit_entity_add`'s scope-chain-filtering pattern exactly.
- Frontend split `LeftNav` into `LeftNav` → `ScopedViewButton` (owns `CommandScopeProvider`) → `ViewButton` (owns `useContextMenu` + dispatch). Mirrors `PerspectiveTabBar` → `ScopedPerspectiveTab` → `PerspectiveTab`.
- Pre-existing unrelated test failure `cross_cutting_context_menu_is_ordered_and_grouped` comes from in-progress `context_menu_group`/`context_menu_order` work in the same tree (types.rs, entity_commands.rs, commands/mod.rs, attachment.yaml). Not caused by this change.