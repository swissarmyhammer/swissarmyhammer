---
depends_on:
- 01KTCQFH7AEQDZD0QETSMCMGP0
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff9a80
project: ui-command-cleanup
title: Card H — Remove the view.switch:${id} client indirection
---
## What
Remove the client-minted `view.switch:${id}` command indirection in `apps/kanban-app/ui/src/components/views-container.tsx`. Today views-container DEFINES a non-backend `view.switch:${id}` command per view whose execute re-dispatches the canonical `view.set {args:{view_id}}`. The canonical `view.set` already lives in `builtin/plugins/ui-commands/index.ts`, and the palette "Switch to View" rows come from Rust `emit_view_switch` — so the client-minted ids are pure indirection.

Approach:
- Stop minting `view.switch:${id}` client-side. The per-view scope bookkeeping that the minted id carried must be handled WITHOUT a client command id — e.g. dispatch the canonical `view.set` with `args.view_id` directly from the view tab/scope, passing the view id as an arg (not as a command-id suffix).
- Keep canonical `view.set` (ui-commands) and Rust `emit_view_switch` (the palette row source) unchanged.
- Confirm what per-view scope state the minted id was tracking and relocate that bookkeeping into the view container's presentation layer (scope provider / focus), not a command id.

## Acceptance Criteria
- [x] No `view.switch:${id}` ids are minted client-side in views-container.tsx.
- [x] Switching views still dispatches canonical `view.set {args:{view_id}}`; palette "Switch to View" rows still come from `emit_view_switch`.
- [x] Per-view scope bookkeeping preserved via presentation, not a client command id.

## Tests
- [x] UI: add/extend a views-container test asserting selecting a view dispatches `view.set` with the right `view_id` arg and NO `view.switch:*` id is registered.
- [x] UI: a test asserting palette "Switch to View" rows still render from the Rust-emitted source.
- [x] Relevant vitest files green.

## Workflow
- Use `/tdd` — failing tests first, then implement. Automated tests only.

## Implementation Notes (done)
- `views-container.tsx`: removed `ViewsCommandScope` entirely (the per-view `view.switch:${id}` CommandDef minting + its `CommandScopeProvider` + `useDispatchCommand`). ViewsContainer is now just ViewsProvider + flex layout (LeftNav + children). Per-view scope bookkeeping was already in presentation: LeftNav's `ScopedViewButton` carries the `view:{id}` moniker and `ViewButton` dispatches canonical `view.set {args:{view_id}}` directly.
- New `views-container.view-set.test.tsx`: mounts the REAL ViewsContainer → ViewsProvider → LeftNav stack; pins (1) selecting a view dispatches `view.set` with `args.view_id`, (2) no `view.switch:*` id anywhere in the scope chain (red→green).
- New `view-switch-commands.retired.node.test.ts`: static architectural guard (plugin-owned-guard scaffold) — no client source may define a `view.switch*` command id (red→green; detector unit-proven incl. the template-literal shape the removed code used).
- `views-container.test.tsx`: "registers view.switch commands" inverted to "mints no view.switch:* commands even when views exist" (red→green).
- `command-palette.test.tsx`: new pin — a Rust-shaped `emit_view_switch` row (`{id:"view.set", name:"Switch to Board"}`) renders from the registry seam (`useCommandList`).
- No Rust changes; `view.set` plugin command and `emit_view_switch` untouched.