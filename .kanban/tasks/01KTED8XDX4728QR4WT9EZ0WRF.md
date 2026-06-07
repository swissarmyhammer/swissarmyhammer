---
depends_on:
- 01KTCQFH7AEQDZD0QETSMCMGP0
position_column: todo
position_ordinal: db80
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
- [ ] No `view.switch:${id}` ids are minted client-side in views-container.tsx.
- [ ] Switching views still dispatches canonical `view.set {args:{view_id}}`; palette "Switch to View" rows still come from `emit_view_switch`.
- [ ] Per-view scope bookkeeping preserved via presentation, not a client command id.

## Tests
- [ ] UI: add/extend a views-container test asserting selecting a view dispatches `view.set` with the right `view_id` arg and NO `view.switch:*` id is registered.
- [ ] UI: a test asserting palette "Switch to View" rows still render from the Rust-emitted source.
- [ ] Relevant vitest files green.

## Workflow
- Use `/tdd` — failing tests first, then implement. Automated tests only.