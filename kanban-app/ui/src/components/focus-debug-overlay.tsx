/**
 * `<FocusDebugOverlay>` — visual decorator that paints a dashed border and
 * coordinate label on top of a spatial primitive's host box.
 *
 * This is a developer aid, not production chrome. It renders only when
 * `useFocusDebug()` returns `true` — controlled by the
 * `<FocusDebugProvider>` mounted at the App root. When the spatial-nav
 * project lands and the overlay is no longer needed, flip
 * `enabled={false}` at the provider site (App.tsx and the quick-capture
 * window) — or pull the provider entirely. Either path causes consumers
 * (`<FocusLayer>`, `<FocusZone>`, `<FocusScope>`) to skip rendering this
 * component, so its existence has zero DOM cost when off.
 *
 * # Why per-primitive
 *
 * Each spatial primitive composes this overlay inside its host `<div>` so
 * the dashed border lines up exactly with the registered rect — the same
 * box `getBoundingClientRect()` returns on the host element. Layered,
 * nested cases (a `<FocusLayer>` containing a `<FocusZone>` containing a
 * `<FocusScope>`) draw three concentric dashed boxes in three colours so
 * the spatial-nav hierarchy is visually distinguishable at a glance.
 *
 * # Color coding
 *
 * - `kind="layer"` → red dashed border (`border-red-500/70`).
 * - `kind="zone"` → blue dashed border (`border-blue-500/70`).
 * - `kind="scope"` → emerald dashed border (`border-emerald-500/70`).
 *
 * # Pointer events
 *
 * The overlay is `pointer-events: none` so it never intercepts clicks,
 * right-clicks, or hovers from the host primitive. Click routing (e.g.
 * `spatial_focus`, `setFocus`, the entity-focus side effects) is unaffected.
 *
 * # Coordinate refresh
 *
 * Rect refresh runs on `requestAnimationFrame` while the overlay is
 * mounted — a perf-untuned aid that captures rect changes from any
 * source (resize, scroll, layout shift). Once
 * `01KQ9XBAG5P9W3JREQYNGAYM8Y` (rects-on-scroll subscription) lands, the
 * rAF loop should be replaced with a subscription to the same scroll/
 * resize observers the production rect-tracking already uses. See the
 * inline TODO below.
 */

import { useContext, useEffect, useRef, useState, type RefObject } from "react";
import { cn } from "@/lib/utils";
import {
  FocusLayerZTierContext,
  OVERLAY_OFFSET_ABOVE_TIER,
} from "@/components/focus-layer-z-tier-context";

/**
 * Which spatial primitive owns this overlay. Drives the colour-coded
 * border and the `${kind}:` prefix on the label.
 */
export type FocusDebugKind = "layer" | "zone" | "scope";

/**
 * Props for `<FocusDebugOverlay>`.
 */
export interface FocusDebugOverlayProps {
  /** Which spatial primitive owns this overlay. */
  kind: FocusDebugKind;
  /**
   * Human-readable label printed in the overlay's top-left corner.
   *
   * For zones and scopes this is the primitive's `Moniker` (e.g.
   * `"task:01ABC"`); for layers it is the `LayerName` (e.g. `"window"`).
   */
  label: string;
  /**
   * Ref to the host element whose box this overlay decorates.
   *
   * The overlay reads `hostRef.current.getBoundingClientRect()` on every
   * animation frame to keep its label coordinates in sync with the host's
   * live rect. When `kind === "layer"`, the rect is intentionally not
   * shown — layers are pure context providers and have no rect of their
   * own; the overlay still mounts so the dashed border and the
   * `layer:<name>` label render against the wrapper div the layer
   * supplies in debug mode.
   */
  hostRef: RefObject<HTMLElement | null>;
}

/**
 * Border / text classes per `FocusDebugKind`.
 *
 * Hard-coded so Tailwind's just-in-time scanner picks the colours up at
 * build time (constructing class names dynamically would defeat that).
 */
const KIND_CLASSES: Record<
  FocusDebugKind,
  { border: string; labelBg: string; labelText: string }
> = {
  layer: {
    border: "border-red-500/70",
    labelBg: "bg-red-500/10",
    labelText: "text-red-500",
  },
  zone: {
    border: "border-blue-500/70",
    labelBg: "bg-blue-500/10",
    labelText: "text-blue-500",
  },
  scope: {
    border: "border-emerald-500/70",
    labelBg: "bg-emerald-500/10",
    labelText: "text-emerald-500",
  },
};

/**
 * Renders the dashed-border + coordinate-label debug decorator.
 *
 * Two stacked absolutely-positioned `<span>` elements:
 *
 *   1. The dashed border, filling the host's content box (`inset-0`).
 *   2. The label pinned to the host's top-left corner, with a tiny
 *      monospace font so it does not overflow small primitives.
 *
 * Both spans carry `pointer-events: none` so click routing on the host
 * is unaffected. The outer wrapper carries `data-debug={kind}` for stable
 * test selectors.
 */
export function FocusDebugOverlay({
  kind,
  label,
  hostRef,
}: FocusDebugOverlayProps) {
  const classes = KIND_CLASSES[kind];

  const [rect, setRect] = useState<DOMRect | null>(null);
  // Refs for the rAF loop. Held outside React state so each frame's
  // re-read does not allocate.
  const frameRef = useRef<number | null>(null);

  useEffect(() => {
    let cancelled = false;

    /**
     * Read the host rect on every animation frame and push it into
     * `rect` state when the rounded x / y values changed. The visible
     * label only shows the (x, y) coordinate pair, so width / height are
     * intentionally NOT part of the equality short-circuit — that keeps
     * the overlay quiet when the host's content reflows in place
     * (dimensions move but the top-left does not). The internal `rect`
     * state still holds full DOMRect (the kernel uses width / height for
     * its own bookkeeping); they are simply not used for either the
     * visible label or the re-render gate.
     *
     * TODO(01KQ9XBAG5P9W3JREQYNGAYM8Y): swap this rAF poll for a
     * subscription to the same scroll/resize observers that
     * `useTrackRectOnAncestorScroll` will expose once the rects-on-scroll
     * ticket lands. The poll is correct but burns a frame per overlay.
     */
    const tick = () => {
      if (cancelled) return;
      const node = hostRef.current;
      if (node) {
        const next = node.getBoundingClientRect();
        setRect((prev) => {
          if (
            prev !== null &&
            Math.round(prev.x) === Math.round(next.x) &&
            Math.round(prev.y) === Math.round(next.y)
          ) {
            return prev;
          }
          return next;
        });
      }
      frameRef.current = requestAnimationFrame(tick);
    };

    frameRef.current = requestAnimationFrame(tick);

    return () => {
      cancelled = true;
      if (frameRef.current !== null) {
        cancelAnimationFrame(frameRef.current);
        frameRef.current = null;
      }
    };
  }, [hostRef]);

  // Build the label text. Layers don't have a meaningful rect of their
  // own (the wrapper div the layer supplies in debug mode is purely a
  // host for the overlay), so we omit coordinates for them. For zones
  // and scopes we show only the (x, y) pair — width / height were
  // visual noise and have been deliberately dropped.
  const labelText =
    kind === "layer" || rect === null
      ? `${kind}:${label}`
      : `${kind}:${label} (${Math.round(rect.x)},${Math.round(rect.y)})`;

  // Layer-aware z-index: read the enclosing `<FocusLayer>`'s tier from
  // context and offset by 5 so the overlay paints just above its
  // layer's modal content but below the next layer's overlays. An
  // inline `style` is required because Tailwind cannot generate
  // classes for runtime-computed values; the previous hardcoded
  // `z-50` is removed because it placed every overlay at the same
  // height regardless of layer membership, causing window-root
  // overlays to bleed across modal surfaces.
  const tier = useContext(FocusLayerZTierContext);

  return (
    <span
      data-debug={kind}
      aria-hidden="true"
      className="pointer-events-none absolute inset-0"
      style={{ zIndex: tier + OVERLAY_OFFSET_ABOVE_TIER }}
    >
      <span
        className={cn(
          "absolute inset-0 border border-dashed pointer-events-none",
          classes.border,
        )}
      />
      <span
        className={cn(
          "absolute left-0 top-0 px-1 text-[9px] font-mono leading-none pointer-events-none",
          classes.labelBg,
          classes.labelText,
        )}
      >
        {labelText}
      </span>
    </span>
  );
}
