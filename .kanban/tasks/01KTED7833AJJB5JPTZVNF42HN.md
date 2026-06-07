---
depends_on:
- 01KTED5F8DQ2XH5BB0WK1MRR3P
position_column: todo
position_ordinal: d780
project: ui-command-cleanup
title: Card D — Move field.edit/editEnter + pressable.activate/activateSpace to plugins + handler bus
---
## What
Move the field-edit and pressable-activate command DEFINITIONS out of React and into plugins, with their webview behaviors routed through the handler bus (Card B).

- `apps/kanban-app/ui/src/components/fields/field.tsx`: `field.edit` / `field.editEnter` (enter field edit mode) — WEBVIEW behavior.
- `apps/kanban-app/ui/src/components/pressable.tsx`: `usePressCommands` defining `pressable.activate` / `pressable.activateSpace` (call local `onPress`) — WEBVIEW behavior.

Approach:
- Define `field.edit`, `field.editEnter`, `pressable.activate`, `pressable.activateSpace` in a plugin — likely the existing `builtin/plugins/ui-commands/index.ts` (these are generic UI-surface commands) — with id/name/keys/scope; no backend op, marked "handled in webview".
- In field.tsx and pressable.tsx, replace the client-side command defs / `usePressCommands` def-building with `registerWebviewCommandHandler(id, handler)` registrations keyed by the plugin ids. The components keep owning the edit-mode and onPress logic as handlers.
- Preserve the activation contracts (Space/Enter) that existing tests pin.

## Acceptance Criteria
- [ ] `field.edit`, `field.editEnter`, `pressable.activate`, `pressable.activateSpace` are defined by a plugin; field.tsx and pressable.tsx no longer DEFINE them.
- [ ] field.tsx / pressable.tsx register webview handlers keyed by those ids; dispatching each runs the original behavior via the bus.
- [ ] Space/Enter activation and field-edit-enter behavior unchanged.
- [ ] GUARD (presentation-only invariant): the edit-mode toggle and `onPress` handlers are pure presentation (local state / DOM focus only). field.tsx and pressable.tsx must NOT import `@/lib/mcp-transport`; any durable effect routes via `useDispatchCommand`. `webview-command-bus.guard.node.test.ts` stays green.

## Tests
- [ ] UI: extend `apps/kanban-app/ui/src/components/fields/field.enter-edit.browser.test.tsx` to assert `field.edit`/`field.editEnter` dispatch through the bus into edit mode.
- [ ] UI: extend `apps/kanban-app/ui/src/components/pressable.test.tsx` to assert `pressable.activate`/`pressable.activateSpace` invoke `onPress` via the bus.
- [ ] Plugin e2e: the chosen plugin registers the four ids with expected metadata.
- [ ] `webview-command-bus.guard.node.test.ts` green with field.tsx and pressable.tsx as registration sites.
- [ ] Relevant vitest files green.

## Workflow
- Use `/tdd` — failing tests first, then implement. Automated tests only.