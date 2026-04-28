---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffd780
project: spatial-nav
title: Focus debug overlay — dashed border + coordinate label on every Layer/Zone/Scope, gated by useFocusDebug
---
## What

Add a developer-facing visual debug overlay on every spatial primitive so we can see at a glance whether `<FocusLayer>`, `<FocusZone>`, and `<FocusScope>` are registering at the coordinates we expect. This is a diagnostic aid for the spatial-nav project — it makes the rect-staleness, conditional-remount, and zero-rect bugs the other tickets in this project are chasing visible without needing a test harness.

Every mounted spatial primitive renders, when debug is on:

- A **single-pixel dashed border** colour-coded by primitive kind (so the layer/zone/scope hierarchy is visually distinguishable in nested cases).
- A **small-font label** showing the primitive's moniker (or layer name) and its current `(x, y)` coordinates, positioned at the top-left of the host box.

The whole feature is gated by a single boolean read through a `useFocusDebug` hook that references a root context. For this PR, the root mount turns the flag on; once the spatial-nav project is past its current bug-fixing phase, flipping the prop off becomes a one-line change and the overlay vanishes.

## Where this fits

The three primitives live in:

- `kanban-app/ui/src/components/focus-layer.tsx` — context-only provider, no host div in production. Debug mode requires wrapping a host element so the dashed border has somewhere to paint.
- `kanban-app/ui/src/components/focus-zone.tsx` — host `<div>` in `SpatialFocusZoneBody` (line 526) and `FallbackFocusZoneBody` (line 647).
- `kanban-app/ui/src/components/focus-scope.tsx` — host `<div>` in `SpatialFocusScopeBody` (line 548) and `FallbackFocusScopeBody` (line 684).

The new context lives alongside the other lib-level focus contexts in `kanban-app/ui/src/lib/`.

## Approach

### 1. New context — `kanban-app/ui/src/lib/focus-debug-context.tsx`

Single-flag context, single hook. Tiny file:

```ts
const FocusDebugContext = createContext<boolean>(false);

export function FocusDebugProvider({ enabled, children }: { enabled?: boolean; children: ReactNode }) {
  // Defaults to true so the bare provider is "on" — App.tsx still passes `enabled` explicitly
  // for clarity. Toggle to false at the App mount when the project no longer needs the overlay.
  return <FocusDebugContext.Provider value={enabled ?? true}>{children}</FocusDebugContext.Provider>;
}

export function useFocusDebug(): boolean {
  return useContext(FocusDebugContext);
}
```

Documented behavior: when no provider wraps the primitive (e.g., a test renders a primitive in isolation), the hook returns `false` (the context default) — debug is off.

### 2. Mount `<FocusDebugProvider enabled>` in App.tsx and quick-capture

`kanban-app/ui/src/App.tsx` line 70 — wrap the existing tree:

```jsx
<DiagErrorBoundary>
  <FocusDebugProvider enabled>
    <SpatialFocusProvider>
      <FocusLayer name={WINDOW_LAYER_NAME}>
        ...
```

`kanban-app/ui/src/quick-capture.tsx` (or wherever the quick-capture window mounts its provider stack — see App.tsx:124) — same wrap. The two production windows are the only entry points; tests do not get the overlay (no provider, default `false`).

### 3. Add the debug overlay to each primitive

Single shared component — `kanban-app/ui/src/components/focus-debug-overlay.tsx`:

```tsx
type DebugKind = "layer" | "zone" | "scope";
interface FocusDebugOverlayProps {
  kind: DebugKind;
  /** Moniker for zones/scopes; layer name for layers. */
  label: string;
  /** Host element ref so the overlay reads its current rect. */
  hostRef: RefObject<HTMLElement | null>;
}
```

Inside, the overlay reads its current rect from `hostRef.current.getBoundingClientRect()` on mount and on every animation frame while debug is on (rAF poll). Debug is intentionally a perf-untuned aid — the rAF loop is fine, and the loop terminates on unmount. Once `01KQ9XBAG5P9W3JREQYNGAYM8Y` (rects-on-scroll) lands, the overlay can subscribe to the same scroll/resize observers instead of polling — leave a `TODO` comment pointing to that ticket so the future cleanup is obvious.

The overlay renders absolutely-positioned elements **inside** the host's existing containing block (zones / scopes already merge `relative` into their className — see `focus-zone.tsx:524` and `focus-scope.tsx:537`):

- A `<span className="absolute inset-0 border border-dashed pointer-events-none ...">` for the border.
- A `<span className="absolute left-0 top-0 px-1 text-[9px] font-mono leading-none bg-background/80 ...">` for the label.

Border colour by kind:
- **layer** → `border-red-500/70` + `text-red-500` on the label background.
- **zone** → `border-blue-500/70` + `text-blue-500`.
- **scope** → `border-emerald-500/70` + `text-emerald-500`.

A `data-debug={kind}` attribute on the overlay span gives tests a stable selector.

The label format: `${kind}:${label} (${Math.round(x)},${Math.round(y)}) ${Math.round(w)}×${Math.round(h)}` for zones/scopes. For layers, `layer:${name}` (no rect — layers don't have rects).

### 4. Wire each primitive to render the overlay when `useFocusDebug()` is `true`

- **`<FocusLayer>`**: layer has no host div in production. In debug mode, wrap children in a `<div className="relative">` that hosts the overlay. The wrapper is conditional — when debug is off, the layer continues to return only the context provider with no DOM box, exactly as today (no layout regression).
- **`<FocusZone>` (both body branches)**: render `<FocusDebugOverlay kind="zone" label={moniker} hostRef={ref} />` as a child of the existing host `<div>` (alongside `<FocusIndicator>`). The host already establishes `position: relative` via the `cn(consumerClassName, "relative")` merge.
- **`<FocusScope>` (both body branches)**: same pattern as zone, with `kind="scope"`.

The overlay is rendered only when `useFocusDebug()` returns `true` — rendered as `null` otherwise. No conditional `useEffect` calls, no hook count instability between debug-on and debug-off renders.

### 5. Document the toggle path

Add a brief note at the top of `focus-debug-context.tsx` and in each primitive's docstring explaining how to flip the flag off (App.tsx — `enabled={false}` on the provider — or pull the provider entirely). When the spatial-nav project lands and the overlay is no longer needed, the cleanup is one prop edit per window.

## Acceptance Criteria

All asserted by automated tests below — no manual smoke step.

- [ ] `useFocusDebug()` returns `true` when wrapped in `<FocusDebugProvider enabled>` and `false` when wrapped in `<FocusDebugProvider enabled={false}>` (or no provider at all).
- [ ] When debug is on, every mounted `<FocusZone>` renders a `[data-debug="zone"]` element inside its host div with `border-dashed` in its class list.
- [ ] When debug is on, every mounted `<FocusScope>` renders a `[data-debug="scope"]` element inside its host div with `border-dashed` in its class list.
- [ ] When debug is on, every mounted `<FocusLayer>` wraps its children in a host div containing a `[data-debug="layer"]` element with `border-dashed`. When debug is off, `<FocusLayer>` renders only its context provider with no extra DOM (regression guard for production layout).
- [ ] When debug is on, the label inside the overlay contains the primitive's moniker (or the layer's name) and, for zones/scopes, the current `(x, y, w, h)` coordinates derived from `getBoundingClientRect()` on the host.
- [ ] When debug is off (no provider, or `enabled={false}`), none of the three primitives renders any `[data-debug=…]` element. Zero overhead in production-with-debug-off mode.
- [ ] Border colour differs across the three kinds (`red` / `blue` / `emerald` per the approach) so nested layer / zone / scope are visually distinguishable. Asserted by inspecting the rendered class list, not by pixel comparison.
- [ ] The overlay is `pointer-events: none` so it never intercepts a click or right-click on the host. (Asserted indirectly by an existing click-routing test mounted with debug on still passing.)
- [ ] App.tsx (and quick-capture) mount `<FocusDebugProvider enabled>` in the production tree so the overlay is visible by default for the duration of this project. Toggling to `enabled={false}` at the mount site removes the overlay site-wide.

## Tests

All tests are automated. No manual verification.

### Frontend — `kanban-app/ui/src/lib/focus-debug-context.test.tsx` (new file)

- [ ] `use_focus_debug_returns_true_when_provider_enabled` — render a small consumer wrapped in `<FocusDebugProvider enabled>`, assert the consumer reads `true`.
- [ ] `use_focus_debug_returns_false_when_provider_disabled` — same with `<FocusDebugProvider enabled={false}>`, assert `false`.
- [ ] `use_focus_debug_returns_false_with_no_provider` — render the consumer alone, assert `false` (default).

Test command: `bun run test focus-debug-context.test.tsx` — all three pass.

### Frontend — `kanban-app/ui/src/components/focus-debug-overlay.browser.test.tsx` (new file)

- [ ] `zone_renders_debug_overlay_when_debug_on` — mount `<FocusZone moniker="ui:test">` inside `<FocusDebugProvider enabled>` + spatial provider stack, query `[data-debug="zone"]`, assert it exists, has `border-dashed` in its class list, and its label contains `"ui:test"`.
- [ ] `scope_renders_debug_overlay_when_debug_on` — same for `<FocusScope moniker="ui:test.leaf">`, assert `[data-debug="scope"]` exists with the moniker.
- [ ] `layer_renders_debug_overlay_when_debug_on` — mount `<FocusLayer name="window">` inside `<FocusDebugProvider enabled>`, assert `[data-debug="layer"]` exists and its label contains `"window"`.
- [ ] `no_overlay_when_debug_off` — same primitive trees inside `<FocusDebugProvider enabled={false}>`, assert no `[data-debug]` elements exist.
- [ ] `no_overlay_when_no_provider` — same primitive trees with no `<FocusDebugProvider>` at all, assert no `[data-debug]` elements.
- [ ] `overlay_label_includes_rounded_coordinates` — mount a `<FocusZone>` inside a fixed-position parent at known `(x, y) = (100, 200)`, assert the rendered label text includes `100,200`.
- [ ] `overlay_kind_classes_are_distinct` — mount one of each primitive in a single tree with debug on, assert the three `[data-debug]` elements have distinct border colours in their class lists (e.g. `border-blue-500/70` for zone, `border-emerald-500/70` for scope, `border-red-500/70` for layer).
- [ ] `overlay_does_not_intercept_clicks` — mount a `<FocusScope>` with debug on, fire a click on the host's content area, assert the click handler runs (focus claim fires) — i.e. the overlay's `pointer-events: none` is wired correctly.
- [ ] `layer_renders_no_dom_when_debug_off` — mount `<FocusLayer>` inside `<FocusDebugProvider enabled={false}>`, assert the rendered tree contains no extra wrapper div added by the layer (only the context provider with the children passed through). Regression guard for production layout.

Test command: `bun run test:browser focus-debug-overlay.browser.test.tsx` — all nine pass.

### Frontend — augment `kanban-app/ui/src/App.test.tsx` (or equivalent root smoke test)

- [ ] `app_renders_focus_debug_provider_at_root` — mount `<App>` with the per-test backend, assert at least one `[data-debug="zone"]` element exists in the rendered tree (proves the root provider is wired and at least one zone has the overlay). If `App.test.tsx` doesn't exist, add the assertion to the closest existing root-level browser test.

Test command: `bun run test:browser App` — passes.

## Workflow

- Use `/tdd` — write the context tests and a failing overlay test first, then implement the context and the overlay component, then wire each primitive.
- Single ticket — one feature (debug overlay), one root flag, one shared overlay component, three primitive consumers, two App-level mount sites.
- Keep the overlay component small and pure: no business logic, no Tauri calls, no spatial-focus subscriptions. It reads the host rect via `getBoundingClientRect()` and renders. Once `01KQ9XBAG5P9W3JREQYNGAYM8Y` (rects-on-scroll) lands, leave a `TODO` to swap the rAF poll for a subscription on the same observers — but do not block this PR on that.
