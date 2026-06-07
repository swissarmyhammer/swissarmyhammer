---
depends_on:
- 01KTED5F8DQ2XH5BB0WK1MRR3P
position_column: todo
position_ordinal: d880
project: ui-command-cleanup
title: Card E — Move editor drill-in commands to plugins + handler bus
---
## What
Move the three editor "drill-in" command DEFINITIONS out of React and into a plugin, routing their CM6/editor-focus behaviors through the handler bus (Card B).

Sites:
- `apps/kanban-app/ui/src/components/perspective-tab-bar.tsx` — `filter_editor.drillIn` (focus the CM6 filter editor).
- `apps/kanban-app/ui/src/components/ai-prompt-composer.tsx` — `ui.ai-panel.composer.drillIn`.
- `apps/kanban-app/ui/src/components/ai-elements/elicitation.tsx` — `ui.ai-panel.elicitation.field.drillIn:${key}` (note the per-field dynamic id suffix).

Approach:
- Define `filter_editor.drillIn`, `ui.ai-panel.composer.drillIn`, and the elicitation field drill-in in a plugin (likely `builtin/plugins/ui-commands/index.ts`), with id/name/keys/scope; no backend op, marked "handled in webview".
- For the elicitation per-field dynamic id (`...drillIn:${key}`): the plugin defines the base command; the dynamic key is passed as an ARG at dispatch (the bus handler reads the field key from ctx.args) — do NOT mint one plugin command per field. Confirm whether the existing palette/keymap needs the suffix or whether arg-passing suffices; document the decision in the card's implementation notes.
- Replace the client-side defs with `registerWebviewCommandHandler` registrations; the editors keep owning their CM6 focus logic.

## Acceptance Criteria
- [ ] `filter_editor.drillIn`, `ui.ai-panel.composer.drillIn`, and the elicitation field drill-in are plugin-defined; the three components no longer DEFINE them.
- [ ] Editor focus (CM6 filter, composer, elicitation field) still occurs on drill-in, via the bus.
- [ ] The elicitation per-field variation is expressed as a dispatch ARG, not N minted command ids.
- [ ] GUARD (presentation-only invariant): drill-in handlers only focus a live editor instance (no durable mutation). perspective-tab-bar.tsx, ai-prompt-composer.tsx, and ai-elements/elicitation.tsx must NOT import `@/lib/mcp-transport`. `webview-command-bus.guard.node.test.ts` stays green.

## Tests
- [ ] UI: extend `apps/kanban-app/ui/src/components/perspective-tab-bar.filter-enter.spatial.test.tsx` (filter drill focuses CM6), `apps/kanban-app/ui/src/components/ai-panel-elicitation.spatial.test.tsx` (elicitation field drill focuses the right field by key), and add/extend an ai-prompt-composer test for composer drill-in.
- [ ] Plugin e2e: the three drill-in ids are registered with expected metadata.
- [ ] `webview-command-bus.guard.node.test.ts` green with the three drill-in components as registration sites.
- [ ] Relevant vitest files green.

## Workflow
- Use `/tdd` — failing tests first, then implement. Automated tests only.