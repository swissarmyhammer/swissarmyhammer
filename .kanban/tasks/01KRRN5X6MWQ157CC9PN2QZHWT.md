---
assignees:
- claude-code
depends_on:
- 01KRRN5HWYA0Z6P7H2BNS1E33B
position_column: todo
position_ordinal: '8980'
project: ai-panel
title: AiPanelContainer — dock the panel into the main layer, collapsible and resizable
---
## What
Place `AiPanel` into the app layout on the main (window) layer.

- New `apps/kanban-app/ui/src/components/ai-panel-container.tsx`. Hosts `AiPanel`, docked on the RIGHT of the main layer — a sibling of `ViewsContainer`, inside `WindowContainer`, OUTSIDE the inspector stack.
- Wire it into `apps/kanban-app/ui/src/App.tsx`'s container hierarchy at that position.
- Collapsible: expose open-state and a toggle prop (the toggle command comes in a later task). Draggable width. Panel-open and width state persist per board in `UIState`.
- The quick-capture window never shows the panel (guard on `IS_QUICK_CAPTURE`).

## Acceptance Criteria
- [ ] `AiPanelContainer` renders `AiPanel` right-docked, as a sibling of `ViewsContainer` inside `WindowContainer`.
- [ ] The panel collapses/expands and its width is draggable; open-state and width persist per board in `UIState`.
- [ ] The panel does not render in the quick-capture window.
- [ ] `npm run build` in `apps/kanban-app/ui` succeeds.

## Tests
- [ ] Vitest browser/component test: panel collapses and expands; collapsed state persists across a remount (reads back from `UIState`).
- [ ] Test: width drag updates and persists.
- [ ] Test: with `IS_QUICK_CAPTURE`, the panel is absent.
- [ ] `npm test` in `apps/kanban-app/ui` is green.

## Workflow
- Use `/tdd` — write the collapse/persist and quick-capture-absence tests first.