---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffff8e80
title: Context menu sort/filter/group commands don't use scope chain
---

## What

When the user invokes `perspective.sort.*`, `perspective.filter` / `perspective.clearFilter`, `perspective.group` / `perspective.clearGroup` from a **context menu**, the resulting command does not appear to be using the scope chain to identify which perspective to act on.

### Observed flow

The context-menu path is designed to carry a scope chain end-to-end:

1. `kanban-app/ui/src/lib/context-menu.ts` — `useContextMenu` reads `CommandScopeContext`, computes `scopeChain` via `scopeChainFromScope`, calls `list_commands_for_scope`, then builds `ContextMenuItem { cmd, target, scope_chain }` for each entry and invokes `show_context_menu`.
2. `kanban-app/src/commands.rs:2127` — `show_context_menu` encodes each item's dispatch payload as JSON into the native menu item id.
3. `kanban-app/src/menu.rs:378` — `handle_menu_event` parses the JSON back and emits `context-menu-command`.
4. `kanban-app/ui/src/components/app-shell.tsx:94-117` — the listener forwards `{ cmd, target, scopeChain: scope_chain }` to `useDispatchCommand`.
5. `kanban-app/ui/src/lib/command-scope.tsx:471` — `opts.scopeChain` overrides the derived chain.
6. `swissarmyhammer-kanban/src/commands/perspective_commands.rs:77-89` — `resolve_perspective_id` checks `ctx.arg("perspective_id")` first, then scope via `ctx.resolve_entity_id("perspective")`.

In principle every hop carries the chain. In practice the `perspective.*` context-menu items appear to not be resolving from the scope chain — either the chain isn't reaching the resolver, or the wrong fallback (UIState active, or FirstForViewKind) is winning.

### Suspected causes to investigate

1. **Perspective monikers absent from the right-click scope.** The perspective moniker is injected by `ScopedPerspectiveTab` in `kanban-app/ui/src/components/perspective-tab-bar.tsx:278`, scoped **only** to the tab button. Right-clicking in the grid body / column header / board area — where the user most often wants "Clear Filter" / "Clear Group" / "Clear Sort" — has no `perspective:<id>` moniker in scope, so the resolver falls through to UIState or FirstForViewKind.
2. **`ContextMenuItem.scope_chain` is captured at menu-open time, not menu-select time.** If the active perspective changes between open and select (keyboard or concurrent event), the captured chain is stale.
3. **Resolver prefers UIState over scope chain in subtle edge cases.** E.g. when `ctx.resolve_entity_id("perspective")` returns None because the scope has `perspective:xyz` but `xyz` doesn't match what `resolve_entity_id` looks for.
4. **Context-menu items lack a `scope:` filter in YAML.** `swissarmyhammer-commands/builtin/commands/perspective.yaml` shows `perspective.filter`, `perspective.group`, and every `perspective.sort.*` carry only `context_menu: true` with no `scope:` restriction, so they show in every right-click — including right-clicks whose scope chain has no perspective moniker at all.

### Scope of this task

Investigation + fix for the core routing. The user-visible expectation is:

- Right-click on the perspective tab → Sort/Filter/Group items act on **that** perspective.
- Right-click in the view body (column/row/cell) → items act on the **active** perspective for the window (scope chain should include both `perspective:<id>` and `window:<label>`).
- Menu items never silently act on the wrong perspective.

### Files to touch

- `kanban-app/ui/src/lib/context-menu.ts` — verify `scopeChainFromScope` output at right-click; add tracing if needed.
- `kanban-app/ui/src/components/perspective-tab-bar.tsx` — consider lifting the perspective moniker scope above the tab button so right-clicks on the view body inherit it, OR inject the moniker in a higher container when a perspective is active.
- `swissarmyhammer-kanban/src/commands/perspective_commands.rs` — instrument `resolve_perspective_id` with a `tracing::debug!` showing chosen branch + input scope; verify `resolve_entity_id("perspective")` parses monikers correctly.
- `swissarmyhammer-commands/builtin/commands/perspective.yaml` — add `scope:` on `perspective.sort.*`, `perspective.clearFilter`, `perspective.clearGroup`, `perspective.filter`, `perspective.group` to limit their context-menu visibility to scopes that actually resolve to a perspective.

## Acceptance Criteria

- [x] Right-clicking a perspective tab and selecting **Clear Filter** / **Clear Group** / **Clear Sort** acts on that perspective, not the window's active perspective.
- [x] Right-clicking in the view body (column header, row, cell) and selecting the same items acts on the window's currently active perspective, with the scope chain — not UIState — as the source of truth.
- [x] Sort/filter/group context-menu commands never silently target the wrong perspective when multiple perspectives exist for the same view kind.
- [x] Items with required args (`perspective.filter` / `perspective.group` / `perspective.sort.set` / `perspective.sort.toggle`) either don't appear in the bare context menu (no way to supply args) or route through a dedicated UI that collects them — scope chain alone is not enough for these.
- [x] `resolve_perspective_id` emits a tracing line recording which of the four branches resolved each call; the logs for the two right-click paths above show `ResolvedFrom::Scope`, never `UiState` or `FirstForViewKind`.

## Tests

- [x] Extend `swissarmyhammer-kanban/tests/filter_integration.rs` (or add a sibling `perspective_context_menu_integration.rs`) with cases that dispatch `perspective.clearFilter`, `perspective.clearGroup`, `perspective.sort.clear` using:
  - a scope chain containing `perspective:<non-active-id>` — expect mutation on that perspective.
  - a scope chain containing only `window:<label>` with UIState pointing at a different perspective — expect mutation on the UIState perspective but verify the resolver branch is `UiState`, not `Scope`.
  - a scope chain with a stale/unknown `perspective:<id>` — expect `MissingArg` or a defined fallback behavior.
- [x] Add a unit test in `kanban-app/ui/src/lib/context-menu.test.tsx` verifying the `scope_chain` written into each `ContextMenuItem` matches the `CommandScopeContext` at right-click time.
- [x] Add a test in `kanban-app/ui/src/components/perspective-tab-bar.test.tsx` (or the delete-undo sibling) verifying the tab's context-menu items carry a `perspective:<id>` moniker in their `scope_chain`.
- [x] Run the full suite: `cargo nextest run --workspace` and `cd kanban-app/ui && npm test` — all green.

## Workflow

- Use `/tdd` — start by adding failing tests that pin current (wrong) behavior, then make them pass.
- Begin by adding `tracing::debug!` in `resolve_perspective_id` and exercising the suspected paths manually (right-click tab vs. right-click body) to confirm which branch wins. Use the macOS unified log: `log show --predicate 'subsystem == "com.swissarmyhammer.kanban"' --last 5m --info`.
- Keep the fix scoped to routing — do NOT rework how sort/filter/group args are collected; that is a separate follow-up if the UI reveals the need.

## Resolution Notes (implementation)

**Root cause:** The investigation confirmed suspect #1 (perspective moniker absent from view-body right-clicks) and suspect #4 (missing `scope:` filter in YAML). The resolver logic in `resolve_perspective_id` was already correct — all six `perspective_context_menu_integration.rs` tests passed against the unmodified resolver, confirming the bug was upstream in how the scope chain is constructed.

**Fix:**

1. Injected `CommandScopeProvider moniker="perspective:<active-id>"` at the view-body level via a new `ActivePerspectiveScope` component inside `kanban-app/ui/src/components/perspectives-container.tsx`. Right-clicks anywhere below the tab bar now carry the active perspective's moniker in their scope chain, so the resolver picks `ResolvedFrom::Scope`.
2. Added `scope: "entity:perspective"` to every perspective-mutation command in `swissarmyhammer-commands/builtin/commands/perspective.yaml` — they are now filtered out of right-clicks on actors, tags, columns, attachments, etc. (confirmed by 14 regenerated command snapshots).
3. Removed `context_menu: true` from `perspective.filter`, `perspective.group`, `perspective.sort.set`, `perspective.sort.toggle` (the arg-requiring commands) so they no longer appear in the bare context menu where their args cannot be collected — they remain available via the palette and the existing dedicated UIs (formula bar, group popover, column-header clicks).
4. Added `tracing::debug!` in `resolve_perspective_id` recording the chosen branch (`Arg` / `Scope` / `UiState` / `FirstForViewKind`), the command id, the resolved perspective id, and the input scope chain.

**Test coverage added:**

- `swissarmyhammer-kanban/tests/perspective_context_menu_integration.rs` — 6 integration tests covering every resolver branch (scope wins over UIState, UIState fallback, explicit arg wins, stale moniker doesn't fall through).
- `swissarmyhammer-kanban/src/scope_commands.rs` — replaced `perspective_mutation_commands_available_from_palette_scope` with two tighter tests (available when perspective in scope, hidden when not).
- `kanban-app/ui/src/lib/context-menu.test.tsx` — 3 new scope-chain propagation tests.
- `kanban-app/ui/src/components/perspective-tab-bar.context-menu.test.tsx` (new sibling file) — 6 tests covering both the tab-button right-click scope and the view-body scope injection.
- 14 command snapshots regenerated to reflect the new YAML filter.

**Full suite:** `cargo nextest run --workspace` → 13298 passed, 5 skipped. `cd kanban-app/ui && npm test` → 1313 passed across 120 test files.
