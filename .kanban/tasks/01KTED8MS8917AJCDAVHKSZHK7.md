---
depends_on:
- 01KTCQFH7AEQDZD0QETSMCMGP0
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff9980
project: ui-command-cleanup
title: Card G — Consolidate entity.inspect + dedup nav.focus
---
## What
Two related consolidations once nav.* are plugin-owned (Card A).

### 1. Consolidate entity.inspect (three client-side definition sites)
- `apps/kanban-app/ui/src/components/app-shell.tsx` — `buildRootInspectCommand` (root fallback `entity.inspect`): dispatches `ui.inspect {target}`, resolving the focused FQM by `INSPECTABLE_ENTITY_PREFIXES`.
- `apps/kanban-app/ui/src/components/inspectable.tsx` — per-Inspectable-scope `entity.inspect` (dispatch `ui.inspect {target: moniker}`).
- nav-bar inspect site (the navbar's inspect entry).

These are three client-side DEFINITIONS of the same `entity.inspect`. Collapse to a SINGLE plugin-owned `entity.inspect` command (the canonical `ui.inspect` target is already a backend op). Resolve the focused entity SERVER-SIDE rather than in React via `INSPECTABLE_ENTITY_PREFIXES`: the focus kernel knows the focused FQM, so `entity.inspect` with no explicit target should resolve the current focus on the backend (or via the focus service) and inspect it. The per-Inspectable scope still provides its moniker as a dispatch arg when present.

CRITICAL CONTRACT: preserve the inspectable Space-binding shadow contract pinned by `apps/kanban-app/ui/src/components/inspectable.space.browser.test.tsx` — the per-Inspectable Space binding must still shadow correctly. Do not break it when removing the client-side def.

### 2. Dedup nav.focus
Both `apps/kanban-app/ui/src/lib/entity-focus-context.tsx` and `apps/kanban-app/ui/src/lib/spatial-focus-context.tsx` DEFINE `nav.focus` (`setFocus(fq)` → backend `spatial_focus`). Card A registers `nav.focus` as a plugin command routing to `spatial_focus`; remove BOTH client-side definitions, leaving the contexts to dispatch the plugin `nav.focus` id (or call `spatial_focus` as presentation wiring) — exactly one definition.

## Acceptance Criteria
- [x] Exactly one plugin-owned `entity.inspect`; `buildRootInspectCommand`, the inspectable.tsx def, and the nav-bar def are removed as DEFINITIONS. (Note: the nav-bar site was already a `ui.inspect` dispatch through Pressable, not an `entity.inspect` definition — nothing to remove there; the two real definition sites are gone and the new plugin-owned guard prevents recurrence.)
- [x] Focused-entity resolution for inspect-with-no-target happens server-side, not via React `INSPECTABLE_ENTITY_PREFIXES` — the ui-commands plugin resolves the innermost inspectable moniker from the dispatched scope chain.
- [x] `nav.focus` is defined exactly once (plugin, Card A); both context-file definitions are removed. The webview's single execution leg is a webview-bus handler registered by `SpatialFocusProvider` running the snapshot-bearing `actions.focus(fq)` commit.
- [x] The inspectable Space-binding shadow contract still holds (innermost entity leads the dispatched chain; Pressable's scope-gated Space still shadows the global binding).

## Tests
- [x] UI: `apps/kanban-app/ui/src/components/inspectable.space.browser.test.tsx` still passes (shadow contract); extended to assert the single plugin `entity.inspect` path (one backend dispatch, zero webview `ui.inspect`). NOTE: 4 of its tests were failing at HEAD because the harness swallowed the MCP-shaped `set focus` echo — fixed the harness translation as part of this card; the file is fully green now.
- [x] UI: `apps/kanban-app/ui/src/components/nav-bar.inspect-enter.spatial.test.tsx` passes unchanged; `inspector-field.space-inspect.browser.test.tsx` + `board-view.enter-drill-in` Space test + end-to-end Family 4 assert inspect resolves focus server-side from the dispatched chain.
- [x] UI: `nav-focus.command.browser.test.tsx` (bus handler + snapshot-carrying click commit), `entity-focus-context.test.tsx`, and `spatial-focus-context.test.tsx` assert nav.focus is single-sourced and still drives `spatial_focus`.
- [x] Relevant vitest files green (343 tests across the touched files) + new guard `inspect-and-focus-commands.plugin-owned.node.test.ts`; Rust: `builtin_ui_commands_e2e` (18 commands, entity.inspect metadata + 3 server-side-resolution effects) + `full_baseline_e2e` (99 ids) — 127/127 in swissarmyhammer-command-service.

## Workflow
- Use `/tdd` — failing tests first, then implement. Automated tests only.

## Out of scope (pre-existing failures noted, unchanged)
26 UI test files fail identically with and without this card's changes (verified by baseline diff at HEAD prod): avatar, board-selector, the 6 legacy `spatial_drill_in` tests in board-view.enter-drill-in, column-view(.virtualized-nav), entity-card, entity-inspector.*, attachment/avatar displays + select editors, focus-scope, grid-empty-state, grid-view.cursor-ring, inspector.* focus tests, inspectors-container.auto-focus, mention-view, path-monikers.kernel-driven, perspective-tab-bar.filter-migration, entity-focus.kernel-projection, perspective-context.

## Review Findings (2026-06-11 13:23)

### Warnings
- [x] `builtin/plugins/nav-commands/index.ts:47` and `:332` — Stale comments: both still say the webview's two `nav.focus` scope defs (`entity-focus-context.tsx` / `spatial-focus-context.tsx`) "take the execute fast-path and supply the snapshot" and that "their dedup is Card G". Card G deleted those defs; the webview leg is now the `SpatialFocusProvider` webview-bus handler. The comments misdescribe the snapshot mechanism on the app's most load-bearing command — update both to name the bus handler (`registerWebviewCommandHandler("nav.focus", …)` → `actions.focus(fq)`).

### Nits
- [x] `ARCHITECTURE.md:618` — "Handlers are presentation-only — they never touch the MCP transport" now has an exception: the `nav.focus` bus handler completes the kernel `set focus` commit via `focus-mcp.ts` over `command_tool_call` (deliberately, per the rationale comment in `spatial-focus-context.tsx`). Document the focus-kernel-commit exception (or the guard's actual enforcement boundary) so the prose matches the code.
- [x] `builtin/plugins/ui-commands/index.ts:218` — `INSPECTABLE_ENTITY_PREFIXES` now lives outside the ui `src/` tree while Guard B/C's matching list stays in `focus-architecture.guards.node.test.ts`, with only comment-level "keep the two lists in sync". The lists can silently drift (a new entity type added to the guard but not the plugin breaks Space for that type with no test failing). Consider a pinning test that reads the plugin source (same repo) and asserts list equality.