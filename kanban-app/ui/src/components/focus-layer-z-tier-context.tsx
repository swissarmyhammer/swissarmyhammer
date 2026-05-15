/**
 * `FocusLayerZTierContext` — z-index tier baseline for the enclosing
 * `<FocusLayer>`'s descendants.
 *
 * Each `<FocusLayer>` publishes a numeric tier so descendant
 * `<FocusDebugOverlay>` instances can paint their dashed-border
 * decoration at a z-index that respects the spatial-nav layer
 * hierarchy. Without this tier, every overlay would render at a single
 * hardcoded z-index and window-root overlays would paint over modal
 * surfaces (the inspector, the palette).
 *
 * The tier is the layer's *baseline*, not the overlay's own z-index —
 * `<FocusDebugOverlay>` adds a small offset so its overlay paints just
 * above the layer's modal content but below the next layer's overlays.
 *
 * Lives in its own module to avoid a `focus-debug-overlay.tsx` ↔
 * `focus-layer.tsx` import cycle. `<FocusLayer>` already imports
 * `<FocusDebugOverlay>`; if the overlay imported the context from
 * `focus-layer.tsx`, both modules would sit on each side of a cycle.
 * The `LayerFqContext` shares this pattern — see
 * `layer-fq-context.tsx`.
 */

import { createContext } from "react";

/**
 * Z-index tier published by the enclosing `<FocusLayer>`.
 *
 * Default `0` corresponds to "no `<FocusLayer>` ancestor at all" — only
 * relevant in unit tests that mount a primitive in isolation. The
 * resulting overlay z-index of `0 + OVERLAY_OFFSET_ABOVE_TIER` (= 5) is
 * below all production UI but visible against a bare test harness.
 */
export const FocusLayerZTierContext = createContext<number>(0);

/**
 * Offset added to a layer's tier baseline to produce the
 * `<FocusDebugOverlay>`'s computed z-index.
 *
 * Sized so that the overlay paints just above its own layer's modal
 * content (e.g. inspector overlay at 35 sits above SlidePanel at 30)
 * yet still well below the next layer's tier baseline (e.g. inspector
 * overlay at 35 sits below dialog overlay at 55), keeping the
 * lower-layer-overlays < higher-layer-content < higher-layer-overlays
 * invariant.
 *
 * Co-located with the tier context (rather than the per-name tier
 * table in `focus-layer.tsx`) because both `<FocusLayer>` and
 * `<FocusDebugOverlay>` import from this module, avoiding the
 * `focus-debug-overlay.tsx` ↔ `focus-layer.tsx` cycle that would arise
 * if the offset lived next to the table.
 */
export const OVERLAY_OFFSET_ABOVE_TIER = 5;
