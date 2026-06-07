---
depends_on:
- 01KTCQFH7AEQDZD0QETSMCMGP0
position_column: todo
position_ordinal: da80
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
- [ ] Exactly one plugin-owned `entity.inspect`; `buildRootInspectCommand`, the inspectable.tsx def, and the nav-bar def are removed as DEFINITIONS.
- [ ] Focused-entity resolution for inspect-with-no-target happens server-side, not via React `INSPECTABLE_ENTITY_PREFIXES`.
- [ ] `nav.focus` is defined exactly once (plugin, Card A); both context-file definitions are removed.
- [ ] The inspectable Space-binding shadow contract still holds.

## Tests
- [ ] UI: `apps/kanban-app/ui/src/components/inspectable.space.browser.test.tsx` still passes (shadow contract); extend it to assert the single plugin `entity.inspect` path.
- [ ] UI: `apps/kanban-app/ui/src/components/nav-bar.inspect-enter.spatial.test.tsx` and an entity-card inspect test assert inspect resolves focus server-side.
- [ ] UI: `apps/kanban-app/ui/src/components/nav-focus.command.browser.test.tsx`, `apps/kanban-app/ui/src/lib/entity-focus-context.test.tsx`, and `apps/kanban-app/ui/src/lib/spatial-focus-context.test.tsx` assert nav.focus is single-sourced and still drives `spatial_focus`.
- [ ] Relevant vitest files green.

## Workflow
- Use `/tdd` — failing tests first, then implement. Automated tests only.