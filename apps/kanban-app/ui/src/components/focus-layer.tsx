/**
 * `<FocusLayer>` — React peer of the Rust `swissarmyhammer_focus::FocusLayer`.
 *
 * A modal boundary in the spatial-nav graph. Each Tauri window mounts a root
 * layer (`name="window"`) at the App tree's root; modal surfaces such as
 * inspectors, dialogs, and palettes mount their own nested layer so the
 * Rust-side navigator can scope beam search to the active layer's scopes.
 *
 * # Path-monikers identity model
 *
 * Card `01KQD6064G1C1RAXDFPJVT1F46` collapsed the legacy UUID-based
 * `LayerKey` into the unified `FullyQualifiedMoniker`. The layer's FQM
 * is its canonical key — composed from its parent FQM (read from
 * `FullyQualifiedMonikerContext`) plus the `name` segment the consumer
 * declared. There is no `crypto.randomUUID()`: the path IS the key.
 *
 * The layer publishes its own composed FQM to descendants via
 * `<FullyQualifiedMonikerContext.Provider value={layerFq}>` — every
 * descendant `<FocusZone>` / `<FocusScope>` reads that FQM as its
 * parent and composes its own.
 *
 * Lifecycle:
 *   - Mount: composes the layer FQM (root = `/<name>`, nested =
 *     `<parentFq>/<name>`) and invokes `spatial_push_layer(fq, segment,
 *     name, parent)` with the resolved `parent` (explicit prop > nearest
 *     ancestor `FullyQualifiedMonikerContext` > `null` for the window
 *     root).
 *   - Unmount: invokes `spatial_pop_layer(fq)` to clean up the Rust-side
 *     stack.
 *
 * ## What is and is not a layer
 *
 * A surface earns its own `<FocusLayer>` if it supports **multi-element
 * keyboard navigation** — arrow keys move focus between two or more
 * controls inside the surface, and the surface should capture those
 * arrows from anything beneath it. Single-control overlays do not earn
 * a layer because there is nothing for a layer to scope.
 *
 * **Layered surfaces (mount a `<FocusLayer>`):**
 *
 * - `name="window"` — every Tauri webview's React root. One per window.
 * - `name="inspector"` — the inspector panel stack (one layer for all
 *   open panels in a window; each panel is a zone inside that layer).
 * - `name="palette"` — the command palette overlay.
 * - `name="dialog"` — confirm / alert dialogs whose body holds two or
 *   more controls.
 *
 * The rule of thumb: if you would naturally write a `useEffect` that
 * traps `keydown` for ArrowUp / ArrowDown / Tab to keep focus inside
 * your overlay, you want a `<FocusLayer>`.
 */

import { useContext, useEffect, useMemo, useRef, type ReactNode } from "react";
import {
  asLayerName,
  composeFq,
  fqRoot,
  type FullyQualifiedMoniker,
  type SegmentMoniker,
} from "@/types/spatial";
import {
  FullyQualifiedMonikerContext,
  useFullyQualifiedMoniker,
  useOptionalFullyQualifiedMoniker,
} from "@/components/fully-qualified-moniker-context";
import { LayerFqContext } from "@/components/layer-fq-context";
import {
  FocusLayerZTierContext,
  OVERLAY_OFFSET_ABOVE_TIER,
} from "@/components/focus-layer-z-tier-context";
import { useFocusDebug } from "@/lib/focus-debug-context";
import { useSpatialFocusActions } from "@/lib/spatial-focus-context";
import {
  LayerScopeRegistry,
  LayerScopeRegistryContext,
} from "@/lib/layer-scope-registry-context";
import { FocusLayerOverlay } from "@/components/focus-debug-overlay";

// ---------------------------------------------------------------------------
// Z-index tier table for layer-aware debug overlays
// ---------------------------------------------------------------------------

/**
 * Per-layer-name z-index tier baseline.
 *
 * Mirrors each layer's existing modal-content z-index so a debug overlay
 * for a window-root descendant sits below the inspector backdrop (z-20)
 * and SlidePanel (z-30); an inspector descendant sits above the panel
 * but below the palette; a palette descendant sits above its own
 * backdrop (z-50). Adding a new first-class layer is a one-line edit
 * here.
 *
 * The actual debug-overlay z-index is `tier + OVERLAY_OFFSET_ABOVE_TIER`
 * — `<FocusDebugOverlay>` applies the offset so the overlay paints just
 * above its layer's modal content but below the next layer's overlays.
 *
 * Layers not in the table fall back to `parentTier + 20` (see the
 * `myTier` derivation below); the gap is large enough that two unnamed
 * nested layers do not collide with each other or with neighbouring
 * named tiers.
 *
 * Cadence: the named tiers step in increments of 20 so a future
 * first-class layer (e.g. `confirmation-modal`) can slot in between any
 * two of the existing names without renumbering. `palette` deliberately
 * sits at 70 (not 60) so the slot at 60 stays open for that purpose.
 */
const LAYER_Z_TIERS: Readonly<Record<string, number>> = {
  /** Window root — flow content; overlays at 15 (below inspector backdrop). */
  window: 10,
  /** Inspector layer — SlidePanel z-30; overlays at 35 (above panel). */
  inspector: 30,
  /** Dialogs — confirm/alert; overlays at 55 (above inspector, below palette). */
  dialog: 50,
  /** Command palette — backdrop z-50; overlays at 75 (above palette backdrop). */
  palette: 70,
};

// Re-export so consumers that already import from `focus-layer` (e.g.
// the existing `FullyQualifiedMonikerContext`, `LayerFqContext`
// re-exports below) can pick the tier context and overlay-offset
// constant up from the same module. The canonical home for both is
// `focus-layer-z-tier-context.tsx`.
export { FocusLayerZTierContext, OVERLAY_OFFSET_ABOVE_TIER };

// ---------------------------------------------------------------------------
// Re-exports — descendants discover their owning FQM via the shared context
// ---------------------------------------------------------------------------

export {
  FullyQualifiedMonikerContext,
  useFullyQualifiedMoniker,
  useOptionalFullyQualifiedMoniker,
};

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/** Props for `<FocusLayer>`. */
export interface FocusLayerProps {
  /**
   * Layer role and path segment — e.g. `"window"`, `"inspector"`,
   * `"dialog"`, `"palette"`.
   *
   * Doubles as the path segment the layer composes into its FQM (so a
   * `name="inspector"` layer mounted under a `/window` ancestor has the
   * FQM `/window/inspector`) and as the `LayerName` metadata sent to
   * the kernel via `spatial_push_layer`.
   */
  name: SegmentMoniker;
  /**
   * Optional override for the parent layer FQM.
   *
   * Defaults to the nearest ancestor `FullyQualifiedMonikerContext`
   * value. Pass an explicit value when content is portaled out of its
   * React parent (e.g. a dialog rendered into `document.body` whose
   * logical parent layer is the window root rather than the surrounding
   * tree). Pass `null` to deliberately mount this layer at the root,
   * ignoring any ancestor context.
   */
  parentLayerFq?: FullyQualifiedMoniker | null;
  children: ReactNode;
}

/**
 * Mounts a layer in the Rust-side stack and exposes its FQM to
 * descendants via `FullyQualifiedMonikerContext`.
 *
 * The layer FQM is composed deterministically from its parent FQM and
 * the `name` segment — no UUID minting, no per-mount identifier
 * randomness. The push/pop pair fires exactly once for the component's
 * lifetime when the resolved `(name, parent)` tuple does not change; if
 * a caller swaps `name` or `parentLayerFq`, the effect tears the layer
 * down and re-pushes it under the new identity.
 */
export function FocusLayer({ name, parentLayerFq, children }: FocusLayerProps) {
  // Resolve the parent FQM — explicit prop wins (including `null` for
  // "force-root"); if the prop is `undefined`, fall back to the nearest
  // ancestor FQM context (which is `null` when no provider wraps us).
  const ancestorFq = useContext(FullyQualifiedMonikerContext);
  const parent: FullyQualifiedMoniker | null =
    parentLayerFq !== undefined ? parentLayerFq : ancestorFq;

  // Compose the layer FQM. Layer roots (no parent) get `/<name>`;
  // nested layers get `<parentFq>/<name>`. The FQM is the canonical
  // identifier — both spatial registry key and the value descendants
  // read from `FullyQualifiedMonikerContext`.
  const fq = useMemo<FullyQualifiedMoniker>(
    () => (parent === null ? fqRoot(name) : composeFq(parent, name)),
    [parent, name],
  );

  // Compute this layer's z-index tier — first-class names use the
  // table; anything else (custom layers, future names not yet added)
  // falls back to `parentTier + 20`, keeping the inner layer's
  // overlays above the parent's. Read the parent tier from the
  // ancestor `FocusLayerZTierContext` (default `0` if no ancestor).
  const parentTier = useContext(FocusLayerZTierContext);
  const myTier = LAYER_Z_TIERS[name] ?? parentTier + 20;

  const { pushLayer, popLayer, registerLayerRegistry } =
    useSpatialFocusActions();

  // ---------------------------------------------------------------------
  // LayerScopeRegistry — React-side replica of the kernel's per-layer
  // scope set. Step 1 of the spatial-nav redesign (parent card
  // 01KQTC1VNQM9KC90S65P7QX9N1): stand the registry up alongside the
  // existing kernel sync. Both sources of truth coexist for now.
  //
  // Held in a ref so the registry instance is stable across re-renders.
  // If the layer's FQM changes (rare — caller swapped `name` or
  // `parentLayerFq`), the registry is rebuilt from scratch so we don't
  // carry stale entries from the previous layer identity. The push/pop
  // effect below already tears the kernel-side layer down on the same
  // dependency change, so React-side teardown matches kernel teardown.
  // ---------------------------------------------------------------------
  const registryRef = useRef<LayerScopeRegistry | null>(null);
  if (registryRef.current === null || registryRef.current.layerFq !== fq) {
    registryRef.current = new LayerScopeRegistry(fq);
  }

  useEffect(() => {
    // The kernel takes both the segment (path component) and the
    // separate `LayerName` metadata. By convention they are the same
    // string for first-class layers ("window", "inspector"), so we
    // re-tag the segment as a `LayerName` for the second arg. The
    // brands are erased at runtime; this is a pure type-level move.
    const layerName = asLayerName(name);
    pushLayer(fq, name, layerName, parent).catch((err) => {
      console.error("[FocusLayer] push failed", err);
    });
    return () => {
      popLayer(fq).catch((err) => {
        console.error("[FocusLayer] pop failed", err);
      });
    };
  }, [fq, name, parent, pushLayer, popLayer]);

  // Publish the layer's scope registry into the spatial-focus
  // provider's per-layer registry map. The snapshot-driven nav path
  // walks that map at decision time to locate the registry that owns
  // the focused FQM. Re-runs when the layer's FQM changes (the
  // registry instance is rebuilt above; both writes happen in the
  // same render cycle).
  useEffect(() => {
    const registry = registryRef.current;
    if (registry === null) return;
    return registerLayerRegistry(fq, registry);
  }, [fq, registerLayerRegistry]);

  // Debug-overlay branch — see `lib/focus-debug-context.tsx`. When the
  // flag is on, render the layer's debug overlay as a SIBLING of
  // children (Option B in card `01KQCHZW5R0WJXTP4BG67QE0Z7`). When the
  // flag is off, render children directly so production layout is
  // byte-identical to the pre-overlay tree.
  //
  // # Why a sibling overlay instead of wrapping children
  //
  // The first attempt (Option A in the card) wrapped children in a
  // `position: fixed; inset: 0; pointer-events: none` host. That
  // gave the dashed border + label a viewport-sized box to paint
  // against, but `pointer-events` is an INHERITED CSS property —
  // setting `pointer-events: none` on the wrapper made every
  // descendant (the SlidePanel, the inspector body, the entire
  // window root subtree) inherit `none` and become hit-test
  // invisible. `document.elementsFromPoint(...)` then returned only
  // `<body>` / `<html>`, breaking the `column_overlay_does_not_paint
  // _over_inspector_panel` regression guard in
  // `focus-debug-overlay.layer-z.browser.test.tsx`. Real clicks
  // would have failed in production too.
  //
  // The sibling form keeps the `pointer-events: none` declaration
  // confined to the overlay's own subtree (owned by
  // `<FocusLayerOverlay>`), which has no descendants that need
  // pointer events apart from its own `pointer-events: auto`
  // tooltip handle. Descendants of `<FocusLayer>` (the panels, the
  // board, etc.) sit OUTSIDE that subtree and keep their default
  // `pointer-events: auto`.
  //
  // # Why this fixes the original 0×0 bug
  //
  // The previous wrapper was `<div className="relative">` and
  // collapsed to 0×0 whenever a layer's children were entirely
  // out-of-flow (e.g. the inspector layer's only DOM child is a
  // `position: fixed` SlidePanel). `<FocusLayerOverlay>` is
  // self-positioning (`position: fixed; inset: 0`) — viewport-sized
  // regardless of what the children look like — so the dashed
  // border + label paint against a real, viewport-sized box.
  const debugEnabled = useFocusDebug();

  return (
    <FullyQualifiedMonikerContext.Provider value={fq}>
      <LayerFqContext.Provider value={fq}>
        <LayerScopeRegistryContext.Provider value={registryRef.current}>
          <FocusLayerZTierContext.Provider value={myTier}>
            {debugEnabled ? <FocusLayerOverlay name={name} /> : null}
            {children}
          </FocusLayerZTierContext.Provider>
        </LayerScopeRegistryContext.Provider>
      </LayerFqContext.Provider>
    </FullyQualifiedMonikerContext.Provider>
  );
}
