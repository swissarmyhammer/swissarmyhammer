---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffec80
project: spatial-nav
title: 'Focus debug overlay: hide id/coords behind a tooltip on a small hoverable handle'
---
## What

`<FocusDebugOverlay>` (`kanban-app/ui/src/components/focus-debug-overlay.tsx`) currently paints a colored dashed border AND an always-visible top-left label that reads `${kind}:${label} (x,y)` (lines 192–228). With overlays mounted on every Layer / Zone / Scope, the screen is wallpapered with overlapping label badges that obscure the actual UI being debugged.

The dashed border is the part that's actually load-bearing — it shows where each primitive's rect lives. The text (kind, moniker, coordinates) is reference info you only need on demand. Move that text into a hover-revealed tooltip.

### Approach

Edit `kanban-app/ui/src/components/focus-debug-overlay.tsx` only.

1. Replace the always-visible label `<span>` (lines 220–228) with a small hoverable handle: a 10–12px square dot/chip pinned to the top-left, color-matched to the kind via `KIND_CLASSES`. The handle is the only part of the overlay that takes pointer events (`pointer-events: auto`); everything else (border span and the wrapping span) stays `pointer-events: none` so click / right-click / hover routing on the host primitive is unchanged.
2. Wrap the handle in `<Tooltip>` / `<TooltipTrigger asChild>` / `<TooltipContent>` from `@/components/ui/tooltip`. Tooltip content text matches the current `labelText` exactly: `${kind}:${label}` for layers, `${kind}:${label} (x,y)` for zones / scopes when a rect is known.
3. Keep the `data-debug={kind}` attribute on the wrapping span so the existing test selectors continue to find the overlay. The label text the tests assert on must move with the tooltip — when the tooltip is closed, the label substring is in `TooltipContent`'s aria/`data-content` (Radix renders it into an offscreen portal even when closed for accessibility), or the test must hover the handle to open it. Either is acceptable; the chosen approach must be deterministic in jsdom **and** in browser-mode tests.
4. Preserve the layer-aware `zIndex: tier + OVERLAY_OFFSET_ABOVE_TIER` on the wrapper. The handle inherits via the wrapper's stacking context.
5. Preserve the rAF rect-tracking loop (lines 138–185) — coordinates are still computed; they just live in the tooltip now. The TODO referencing `01KQ9XBAG5P9W3JREQYNGAYM8Y` (rects-on-scroll subscription) stays unchanged; this task is independent.

### Visual outcome

  - Overlay-on: dashed colored border around each primitive (unchanged), plus a tiny color-coded dot in the top-left corner. Hover the dot → tooltip pops up with `kind:moniker (x,y)`.
  - Overlay-off: zero DOM cost (already true via `useFocusDebug()` short-circuit at the call sites in `focus-layer.tsx`, `focus-zone.tsx`, `focus-scope.tsx`).

### Why a hover handle and not just `title=`

Radix Tooltip is the project's tooltip primitive (used in `nav-bar.tsx`, `entity-card.tsx`, etc.). HTML `title=` would render after a long delay, can't be styled, and is invisible to e2e tests. Use the existing Tooltip component for consistency.

## Acceptance Criteria
- [x] `<FocusDebugOverlay>` renders a small (~10–12px) color-matched handle pinned to the top-left of its host's content box. The handle carries `pointer-events: auto`; the surrounding wrapper and dashed border keep `pointer-events: none`.
- [x] The handle is wrapped in a Radix Tooltip. Hovering it opens a `TooltipContent` whose text is exactly the current `labelText` value (`${kind}:${label}` for layers, `${kind}:${label} (x,y)` otherwise).
- [x] The dashed border, color coding, layer-aware z-index, and `data-debug={kind}` attribute are unchanged.
- [x] Click routing on the host element is unchanged: clicking on the host's content (NOT on the handle) still reaches the host's click handler, not the overlay. (Existing click-passthrough invariant in `focus-debug-overlay.browser.test.tsx` assertion #4.)
- [x] When `useFocusDebug()` returns `false`, no overlay or handle DOM is mounted (already enforced upstream — verify nothing in this change makes it conditional on the new tooltip subtree).

## Tests
- [x] Update `kanban-app/ui/src/components/focus-debug-overlay.browser.test.tsx`:
  - Existing "label mentions primitive's name / moniker" assertion (#1): change to fire a hover on the handle and assert the tooltip content text matches `kind:moniker`. Use `@testing-library/user-event`'s `hover()` against `[data-debug=…] [data-tooltip-trigger]` (or whatever stable selector the handle exposes).
  - Existing "(x,y) coordinates" assertion (#3): hover the handle and assert tooltip text contains the `"x,y"` substring.
  - Existing "no overlay when provider disabled" assertion (#2): unchanged — still verifies no `[data-debug=…]` mounts.
  - Existing "click passthrough" assertion (#4): unchanged — clicks on host content still reach the host. Add a sub-assertion: clicking on the *handle itself* must NOT reach the host (the handle is the only `pointer-events: auto` region; this is the explicit affordance for hover).
- [x] Update `kanban-app/ui/src/components/focus-debug-overlay.layer-z.browser.test.tsx` only if the z-tier read needs to move — preserve the existing layer-z assertions.
- [x] Run `cd kanban-app/ui && pnpm vitest run src/components/focus-debug-overlay` and confirm green.

## Workflow
- Use `/tdd` — flip the existing browser test's "label always visible" expectation to "label visible only after hovering the handle"; watch it fail; replace the visible `<span>` with the Tooltip-wrapped handle; watch it pass. Click-passthrough assertion stays green throughout.

## Implementation Notes

- **Self-contained TooltipProvider:** `<FocusDebugOverlay>` includes its own `<TooltipProvider delayDuration={0}>` wrapping the Tooltip. Production mounts a `<TooltipProvider>` at `<WindowContainer>` for chrome tooltips, but the *window-layer* `<FocusLayer>` in `App.tsx` sits *outside* `<WindowContainer>` — its layer-kind overlay would have no provider in scope. A local provider keeps the overlay self-contained; nested Radix providers are explicitly supported.
- **Inline width/height on the handle:** the handle uses `style={{ width: 12, height: 12, position: "absolute" }}` in addition to Tailwind classes (`h-3 w-3`) so it has a deterministic 12×12 hit area in the vitest browser project, which mounts components without the `tailwindcss()` plugin.
- **Tests assert via `aria-label`:** the handle's `aria-label` mirrors the tooltip content verbatim, so most label-text assertions read `getAttribute("aria-label")` on the handle (deterministic, no hover round-trip needed). Two new tests (`tooltip_opens_on_handle_hover`, `tooltip_for_layer_kind_shows_kind_and_label`) use `userEvent.hover()` from `vitest/browser` to exercise the real hover→portal path end-to-end.
- **`readOpenTooltipText` strips the `[role="tooltip"]` clone:** Radix renders both the visible `Slottable` and an offscreen `<VisuallyHidden role="tooltip">` clone of the same children; the helper subtracts the latter so `textContent` matches what the user sees, not the doubled accessibility shadow.
