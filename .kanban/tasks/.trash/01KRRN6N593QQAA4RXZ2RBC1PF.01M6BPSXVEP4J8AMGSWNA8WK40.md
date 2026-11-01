---
assignees:
- claude-code
depends_on:
- 01KRRN69YDB2B03RB1N9G6RR3J
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffa80
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
- [x] The panel is a spatial-nav layer with a path-correct moniker under the window layer; focus crosses view-area <-> panel without a cross-layer jump.
- [x] The composer, the model selector, and the per-message action buttons are each individually focusable, reachable by jump-to, and navigable by spatial nav.
- [x] Navigating inside the panel behaves like navigating anywhere else in the app — same primitives, same keymaps.
- [x] `npm run build` in `apps/kanban-app/ui` succeeds.

## Tests
- [x] Spatial-nav tests (`*.spatial.test.tsx`): jump into the panel and land on the composer; jump to the model selector; spatial-nav between panel controls; cross view-area <-> panel and back.
- [x] Test that each interactive element registers a focus scope and a jump code.
- [x] `npm test` in `apps/kanban-app/ui` is green.

## Workflow
- Use `/tdd` — write the jump-into-panel and intra-panel navigation tests first.

## Implementation Notes

### Moniker path used
The panel is registered as a **zone** — a `<FocusScope moniker="ui:ai-panel">` — NOT its own `<FocusLayer>`. A separate layer would make the kernel's layer-boundary guard refuse cardinal navigation between the view area and the panel; the task requires the view-area <-> panel crossing to be clean with no cross-layer jump. So the panel shares the `/window` layer with the board, exactly like `ui:perspective`, `ui:navbar.*`, and `ui:left-nav`.

Because the panel mounts inside `App.tsx`'s window-root `<FocusLayer name="window">`, `<FocusScope>` composes the zone FQM as a PATH through the window layer (via `FullyQualifiedMonikerContext`), not a flat leaf string:

- Panel zone:        `/window/ui:ai-panel`
- Model selector:    `/window/ui:ai-panel/ui:ai-panel.model-selector`
- Scrollback region: `/window/ui:ai-panel/ui:ai-panel.scrollback`
- Composer:          `/window/ui:ai-panel/ui:ai-panel.composer`
- Per-message copy:  `/window/ui:ai-panel/ui:ai-panel.scrollback/ui:ai-panel.message-action:{messageId}:copy`
- Per-message retry: `/window/ui:ai-panel/ui:ai-panel.scrollback/ui:ai-panel.message-action:{messageId}:retry`

The per-message action leaves nest one level deeper (parented at the `ui:ai-panel.scrollback` zone) because the messages physically and structurally live inside the conversation scrollback region — that is the path-correct parent. Their FQMs are still path-descendants of the `ui:ai-panel` zone.

### Key decisions
- **Zone, not layer** — see above. Confirmed by the spatial test `ArrowLeft from the composer crosses into the view area within the same window layer`.
- **Conditional spatial wiring** — `<FocusScope>` / `<Pressable>` throw outside a `<FocusLayer>`. Production (`App.tsx`) always mounts the window layer, but the `AiPanel` View is unit-tested standalone. New `ai-panel-focus.tsx` exposes `<AiPanelFocusScope>` / `<AiPanelPressable>` thin wrappers that render the spatial primitive only when an enclosing `<FocusLayer>` is present (mirroring `perspective-container.tsx`'s `PerspectiveSpatialZone`); otherwise they render the bare host. The no-layer `<AiPanelPressable asChild>` fallback routes through `<Slot.Root>` and composes the outer-injected `onClick` so the model-selector dropdown trigger still opens.
- **Per-message action buttons** — copy (every message with text) + retry (user messages, resends the prompt). The AI panel previously had no per-message actions; they were added as part of this task using the AI Elements `MessageActions` container and `<AiPanelPressable>` leaves.
- **Container test mocks** — `ai-panel-container.test.tsx` now mounts a spatial-aware `AiPanel`, whose module graph transitively imports `@tauri-apps/api/event`. Added the standard `event` / `window` / `plugin-log` stubs so the real `@tauri-apps/api/event` (which reaches into `core` for `transformCallback`) never loads.

### Files changed
- `apps/kanban-app/ui/src/components/ai-panel.tsx` — panel zone, model-selector / scrollback / composer focus scopes, per-message copy/retry action bar.
- `apps/kanban-app/ui/src/components/ai-panel-focus.tsx` (new) — `<AiPanelFocusScope>` / `<AiPanelPressable>` conditional spatial wrappers.
- `apps/kanban-app/ui/src/components/ai-panel.spatial.test.tsx` (new) — 8 spatial-nav tests (zone moniker path, per-control focus scopes, jump-to lands on composer / model selector / copy action, intra-panel nav, cross view-area <-> panel and back).
- `apps/kanban-app/ui/src/components/ai-panel-container.test.tsx` — added `event` / `window` / `plugin-log` mocks for the now-spatial-aware panel import graph.

## Review Findings (2026-05-18 12:42)

Task-mode review. Scope: `ai-panel.tsx`, `ai-panel-focus.tsx`, `ai-panel.spatial.test.tsx`, `ai-panel-container.test.tsx`.

Moniker-path correctness was the crux of this review and is **verified correct**: `<FocusLayer name="window">` publishes `/window`; `<FocusScope moniker="ui:ai-panel">` composes `/window/ui:ai-panel` via `composeFq(parentFq, segment)`; inner controls and per-message actions compose path-descendants one and two levels deeper. Every FQM goes through `composeFq` / `fqRoot` against the ancestor `FullyQualifiedMonikerContext` — nothing registers a flat/leaf moniker. The zone-vs-layer decision is sound: a separate `FocusLayer` would trip the layer-boundary guard and block clean view-area <-> panel cardinal nav, which the task forbids; sharing the `/window` layer matches the `nav-bar.tsx` / `perspective-container.tsx` precedent and is proven by the passing cross-zone tests. All 25 AI-panel tests pass in real Chromium; `npm run build` succeeds; the 4 unrelated full-suite failures are the documented pre-existing tasks `01KRS426Q36ZN3DYBX2S0AS82T` and `01KRVG4QSXPQ2FW5SG61M8EHAP`.

### Nits
- [x] `apps/kanban-app/ui/src/components/ai-panel.spatial.test.tsx:22-24,283` — The file docstring and the first test name say the panel zone FQM is `/window/ai-panel`, but the moniker segment is `ui:ai-panel`, so the real FQM is `/window/ui:ai-panel`. The assertion itself (lines 296-300) composes the correct path, so this is purely a stale comment/name. Update the docstring and the `it(...)` title to say `/window/ui:ai-panel` for accuracy.
  - Resolved: corrected both stale `/window/ai-panel` references in the file docstring (the FocusScope-zone bullet and the layer-boundary paragraph) and the first `it(...)` title to read `/window/ui:ai-panel`. The assertion was already correct; no behavior change. All 8 spatial tests still pass.
- [x] `apps/kanban-app/ui/src/components/ai-panel.tsx:719` — `MESSAGE_ACTION_BUTTON_CLASS` is declared after its sole consumer `MessageActionBar` (line 687). No runtime issue (the const is initialized before any render), but placing a module constant below the component that uses it reads oddly; consider hoisting it above `MessageActionBar` next to the other module-level constants.
  - Resolved: hoisted `MESSAGE_ACTION_BUTTON_CLASS` (with its doc comment) up to immediately above the `MessageActionBarProps` interface, so the constant is now declared before its only consumer `MessageActionBar`. Pure code-ordering move, no behavior change; `npm run build` is clean and the AI-panel suites pass.