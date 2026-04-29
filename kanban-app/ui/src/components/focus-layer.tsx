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
import { useFocusDebug } from "@/lib/focus-debug-context";
import { useSpatialFocusActions } from "@/lib/spatial-focus-context";
import { FocusDebugOverlay } from "@/components/focus-debug-overlay";

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

  const { pushLayer, popLayer } = useSpatialFocusActions();

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

  // Debug-overlay branch — see `lib/focus-debug-context.tsx`. When the
  // flag is on, wrap children in a `<div className="relative">` so the
  // absolutely-positioned dashed border + label have a containing block
  // to paint against. When the flag is off, render children directly so
  // production layout is byte-identical to the pre-overlay tree.
  const debugEnabled = useFocusDebug();
  // Ref outside the conditional so the hook count is stable across
  // debug-on / debug-off renders. The host element is only attached when
  // debug is enabled; when off, the ref simply never receives a node.
  const debugHostRef = useRef<HTMLDivElement | null>(null);

  return (
    <FullyQualifiedMonikerContext.Provider value={fq}>
      <LayerFqContext.Provider value={fq}>
        {debugEnabled ? (
          <div ref={debugHostRef} className="relative">
            <FocusDebugOverlay
              kind="layer"
              label={name}
              hostRef={debugHostRef}
            />
            {children}
          </div>
        ) : (
          children
        )}
      </LayerFqContext.Provider>
    </FullyQualifiedMonikerContext.Provider>
  );
}
