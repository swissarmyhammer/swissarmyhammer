/**
 * `<FocusDebugOverlay>` — visual decorator that paints a dashed border and
 * a hover-revealed coordinate label on top of a spatial primitive's host
 * box.
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
 * # Hover-revealed label
 *
 * The kind / moniker / coordinate label is hidden behind a small
 * (~12px) color-matched handle pinned to the host's top-left corner.
 * Hovering the handle pops a Radix tooltip whose content is exactly the
 * computed `labelText`. With overlays mounted on every Layer / Zone /
 * Scope the screen would otherwise be wallpapered with overlapping label
 * badges that obscure the actual UI being debugged; the dashed border is
 * the part that's load-bearing — the text is reference info you only
 * need on demand.
 *
 * # Pointer events
 *
 * The wrapping span and the dashed-border span are `pointer-events:
 * none` so they never intercept clicks, right-clicks, or hovers from the
 * host primitive. The handle is the *only* `pointer-events: auto`
 * region — it is the explicit affordance for hover. Clicks on the
 * handle itself are stopped at the handle (they do not bubble up to the
 * host's `onClick` / `spatial_focus` path); clicks anywhere else on the
 * host pass through the overlay unchanged.
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

import {
  useContext,
  useEffect,
  useRef,
  useState,
  type MouseEvent,
  type RefObject,
} from "react";
import { cn } from "@/lib/utils";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
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
 * Border / handle classes per `FocusDebugKind`.
 *
 * Hard-coded so Tailwind's just-in-time scanner picks the colours up at
 * build time (constructing class names dynamically would defeat that).
 *
 * - `border` is the dashed-border colour for the rect outline.
 * - `handle` is the solid-fill background for the small hover handle in
 *   the top-left corner. The same hue family as the border so the
 *   handle reads as "the affordance for *this* primitive" at a glance.
 */
const KIND_CLASSES: Record<
  FocusDebugKind,
  { border: string; handle: string }
> = {
  layer: {
    border: "border-red-500/70",
    handle: "bg-red-500/70 ring-1 ring-red-500",
  },
  zone: {
    border: "border-blue-500/70",
    handle: "bg-blue-500/70 ring-1 ring-blue-500",
  },
  scope: {
    border: "border-emerald-500/70",
    handle: "bg-emerald-500/70 ring-1 ring-emerald-500",
  },
};

/**
 * Renders the dashed-border + hover-revealed coordinate-label debug
 * decorator.
 *
 * The structure is three absolutely-positioned elements stacked inside
 * a `pointer-events: none` wrapper:
 *
 *   1. The dashed border, filling the host's content box (`inset-0`).
 *      `pointer-events: none` so it does not intercept anything on the
 *      host.
 *   2. A small (~12px) color-matched square handle pinned to the host's
 *      top-left corner. This is the *only* `pointer-events: auto`
 *      region of the overlay — hover here to reveal the tooltip. Click
 *      events on the handle are stopped at the handle so they do not
 *      reach the host's `onClick` (the handle is the explicit
 *      affordance; spurious clicks while reaching for the tooltip
 *      should not flip focus).
 *   3. A Radix `<TooltipContent>` portalled to the document body when
 *      the tooltip is open. Holds the `kind:label` (layer) or
 *      `kind:label (x,y)` (zone / scope) text.
 *
 * The outer wrapper carries `data-debug={kind}` for stable test
 * selectors. The handle carries `data-debug-handle={kind}` so tests can
 * target it directly when firing hover events.
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

  /**
   * Stop click events on the handle from reaching the host's `onClick`
   * (e.g. `<FocusScope>`'s `spatial_focus` dispatcher). The handle is
   * the only interactive region of the overlay; clicking it is the
   * affordance for opening the tooltip, not for activating the
   * underlying primitive.
   */
  const stopHandleClick = (event: MouseEvent<HTMLSpanElement>) => {
    event.stopPropagation();
  };

  return (
    <span
      data-debug={kind}
      className="pointer-events-none absolute inset-0"
      style={{ zIndex: tier + OVERLAY_OFFSET_ABOVE_TIER }}
    >
      <span
        className={cn(
          "absolute inset-0 border border-dashed pointer-events-none",
          classes.border,
        )}
      />
      {/*
        * Local `<TooltipProvider>` so the overlay does not depend on
        * the application root having mounted one. Production already
        * mounts a `<TooltipProvider>` at `<WindowContainer>` for chrome
        * tooltips, but `<FocusDebugOverlay>` is invoked from
        * `<FocusLayer>` — and the *window* layer mounts above
        * `<WindowContainer>` in `App.tsx`, so its layer-kind overlay
        * sits *outside* that provider. A local provider here keeps the
        * overlay self-contained: it works under any caller, with
        * `delayDuration={0}` so the visible label appears instantly on
        * hover (a developer aid does not need the production 400ms
        * settle delay). Nested `<TooltipProvider>`s are explicitly
        * supported by Radix — the inner one shadows the outer for its
        * own subtree.
        */}
      <TooltipProvider delayDuration={0}>
        <Tooltip>
          <TooltipTrigger asChild>
            <span
              data-debug-handle={kind}
              role="button"
              tabIndex={0}
              aria-label={labelText}
              onClick={stopHandleClick}
              onMouseDown={stopHandleClick}
              className={cn(
                "absolute left-0 top-0 rounded-sm pointer-events-auto cursor-help",
                classes.handle,
              )}
              // Inline width/height so the handle has a deterministic
              // 12×12 hit area even in test environments where
              // Tailwind is not loaded (the kanban-app vitest browser
              // project mounts components without the `tailwindcss()`
              // plugin). Production picks up the same 12×12 from the
              // equivalent Tailwind classes via `<WindowContainer>`'s
              // stylesheet.
              style={{ width: 12, height: 12, position: "absolute" }}
            />
          </TooltipTrigger>
          <TooltipContent side="bottom" align="start" className="font-mono">
            {labelText}
          </TooltipContent>
        </Tooltip>
      </TooltipProvider>
    </span>
  );
}
