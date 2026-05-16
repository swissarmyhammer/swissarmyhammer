---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: 8d80
project: ai-panel
title: AI panel spatial-nav zone, CM6 composer, and bottom-bar status
---
## What
Integrate the AI panel with the app's spatial-navigation and text-editor conventions.

- Register the panel as its own spatial-nav layer/zone, a child of the window root. The zone moniker MUST be a proper path THROUGH the window layer (not a flat leaf) so navigation between the view area and the panel is not treated as a cross-layer jump. (See the path-based-moniker rule — flat monikers cause duplicate-registration ambiguity.)
- Make the `PromptInput` composer a CodeMirror 6 instance using the app's keymap (vim/emacs/CUA), consistent with every other text input in the app — not a plain `<textarea>`.
- Show AI status in the bottom bar: idle / streaming / error.

Spec: `ideas/kanban/ai_panel.md` — Phase 5 "Spatial navigation". Reference: `ideas/kanban/app-architecture.md` (CM6 everywhere), existing `.spatial.test.tsx` tests.

## Acceptance Criteria
- [ ] The AI panel is a spatial-nav layer/zone with a path-correct moniker under the window layer; focus moves between the view area and the panel without a cross-layer jump.
- [ ] The composer is a CM6 instance honoring the active keymap.
- [ ] The bottom bar reflects AI status (idle / streaming / error).
- [ ] `npm run build` in `apps/kanban-app/ui` succeeds.

## Tests
- [ ] Spatial-nav test (`*.spatial.test.tsx`): navigate from the view area into the panel and back; assert no cross-layer jump and correct focus landing.
- [ ] Component test: the composer is CM6 and a keymap motion works inside it.
- [ ] Component test: bottom bar shows `streaming` during a prompt and `idle` after.
- [ ] `npm test` in `apps/kanban-app/ui` is green.

## Workflow
- Use `/tdd` — write the spatial-nav crossing test first.