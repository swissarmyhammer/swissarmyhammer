---
assignees:
- claude-code
depends_on:
- 01KRRN69YDB2B03RB1N9G6RR3J
position_column: todo
position_ordinal: 8b80
project: ai-panel
title: AI panel focus scopes — jump-to and spatial navigation for the panel's controls
---
## What
Every interactive element in the AI panel must participate in the app's focus, spatial-navigation, and jump-to systems — so the user can "jump" into the panel and navigate its controls exactly like the rest of the app.

- Register the panel as its own spatial-nav layer (`FocusLayer`) / zone, a child of the window root. The zone moniker MUST be a proper path THROUGH the window layer, not a flat leaf — flat monikers cause duplicate-registration ambiguity (path-based-moniker rule).
- Give each interactive element in the panel its own focus scope and make it a focusable spatial-nav target, reusing the existing `FocusScope` / `Focusable` / `Pressable` primitives: the composer/input box, the model selector, the per-message action buttons (copy / retry / etc.), and the conversation scrollback region.
- Jump-to: the panel's focusable targets emit jump codes (via the existing `generate_jump_codes` path) so the jump-to overlay can land directly on the composer, the model selector, etc.
- Spatial navigation: cardinal/arrow nav moves between the panel's controls, and crosses cleanly between the view area and the panel without a cross-layer jump.

Reference: `ideas/kanban/app-architecture.md`; existing `focus-scope.*`, `focusable.*`, `pressable.*`, `nav-focus.*`, `spatial-nav-jump-to.spatial.test.tsx`.

## Acceptance Criteria
- [ ] The panel is a spatial-nav layer with a path-correct moniker under the window layer; focus crosses view-area <-> panel without a cross-layer jump.
- [ ] The composer, the model selector, and the per-message action buttons are each individually focusable, reachable by jump-to, and navigable by spatial nav.
- [ ] Navigating inside the panel behaves like navigating anywhere else in the app — same primitives, same keymaps.
- [ ] `npm run build` in `apps/kanban-app/ui` succeeds.

## Tests
- [ ] Spatial-nav tests (`*.spatial.test.tsx`): jump into the panel and land on the composer; jump to the model selector; spatial-nav between panel controls; cross view-area <-> panel and back.
- [ ] Test that each interactive element registers a focus scope and a jump code.
- [ ] `npm test` in `apps/kanban-app/ui` is green.

## Workflow
- Use `/tdd` — write the jump-into-panel and intra-panel navigation tests first.