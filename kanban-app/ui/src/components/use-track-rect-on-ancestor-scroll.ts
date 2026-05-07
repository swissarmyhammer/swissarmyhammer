/**
 * `useTrackRectOnAncestorScroll` — keep a spatial primitive's stored rect
 * in sync with its on-screen position when an ancestor scrolls.
 *
 * # Background
 *
 * `<FocusZone>` and `<FocusScope>` register their host element's
 * `getBoundingClientRect()` with the Rust spatial registry on mount and
 * keep it fresh via a `ResizeObserver`. `getBoundingClientRect()` returns
 * **viewport-relative** coordinates, so a scroll on any ancestor shifts the
 * host's viewport-y while the host's own size never changes —
 * `ResizeObserver` does not fire, and the kernel keeps using the stale
 * mount-time rect. Beam-search then runs on stale geometry and either
 * picks the wrong candidate or none at all.
 *
 * Off-screen virtualised cards (the placeholder rows in `column-view.tsx`)
 * already get correct rects because their registration hook re-runs on
 * `scrollOffset` change. Real-mounted primitives need an equivalent path,
 * which is what this hook provides.
 *
 * # What this hook does
 *
 *   - On mount, walks the host's parent chain to collect every scrollable
 *     ancestor (an element whose computed `overflow-{x,y}` is `auto` or
 *     `scroll`). The walk stops at `document.documentElement`.
 *   - Adds the `window` itself to the listener set so document-level
 *     scrolls (when `<html>`/`<body>` is the scrolling root) are also
 *     observed.
 *   - Attaches a passive `scroll` listener to each, throttled via
 *     `requestAnimationFrame` so a fast scroll produces O(frames) IPC
 *     calls, not O(events).
 *   - On unmount or when any of the inputs (key, host node) flip, detaches
 *     every listener and cancels any pending rAF.
 *
 * # Coordination with `ResizeObserver`
 *
 * Both this hook and the existing `ResizeObserver` write to the same
 * single-FQM entry in the Rust registry via `spatial_update_rect`. The
 * kernel's update is idempotent on `FullyQualifiedMoniker` (a single-key
 * overwrite), so the writes coalesce safely — the latest rect always
 * wins. Both can coexist; they cover orthogonal triggers (size vs
 * ancestor scroll).
 */

import { useEffect } from "react";
import {
  asPixels,
  type FullyQualifiedMoniker,
  type Rect,
} from "@/types/spatial";
import type { LayerScopeRegistry } from "@/lib/layer-scope-registry-context";

/**
 * The subset of `SpatialFocusActions["updateRect"]` this hook needs. Kept
 * narrow so callers do not have to import the full action shape.
 *
 * `sampledAtMs` mirrors the optional third argument on
 * `SpatialFocusActions["updateRect"]` — the dev-mode rect validator
 * uses it to detect stale samples (a rect captured before an unobserved
 * scroll). This hook captures `performance.now()` immediately after
 * `getBoundingClientRect()` and threads it through.
 */
type UpdateRect = (
  fq: FullyQualifiedMoniker,
  rect: Rect,
  sampledAtMs?: number,
) => Promise<void>;

/**
 * Walk an element's parent chain and return every scrollable ancestor.
 *
 * An element counts as scrollable when its computed `overflow-x` or
 * `overflow-y` is `auto`, `scroll`, or `overlay`. The walk stops at
 * `document.documentElement` — `window` is added separately by the
 * caller because the `scroll` event for the document fires on `window`
 * rather than on `<html>`.
 *
 * Pure function; no DOM mutation, no side effects.
 */
function findScrollableAncestors(node: Element): Element[] {
  const out: Element[] = [];
  const root = node.ownerDocument?.documentElement ?? null;
  let cursor: Element | null = node.parentElement;
  while (cursor && cursor !== root) {
    const style = getComputedStyle(cursor);
    const overflowX = style.overflowX;
    const overflowY = style.overflowY;
    if (
      overflowX === "auto" ||
      overflowX === "scroll" ||
      overflowX === "overlay" ||
      overflowY === "auto" ||
      overflowY === "scroll" ||
      overflowY === "overlay"
    ) {
      out.push(cursor);
    }
    cursor = cursor.parentElement;
  }
  return out;
}

/**
 * Mount a per-rAF throttled `scroll` listener on every scrollable
 * ancestor of `nodeRef.current` and on `window`, refreshing the kernel's
 * rect for `key` whenever any of them scrolls.
 *
 * The hook re-runs whenever `key`, `nodeRef`, or `updateRect` flips
 * identity — but in practice all three are stable across the host's
 * lifetime (the key is minted once into a ref by the parent primitive,
 * the node ref is assigned via callback, and `updateRect` is memoised by
 * the spatial-focus context).
 *
 * # Parameters
 *
 * - `nodeRef`: ref to the rendered host element. The hook reads
 *   `.current` lazily so the first render's `null` is tolerated; the
 *   effect bails out when the ref is empty.
 * - `key`: the `FullyQualifiedMoniker` to push rect updates against.
 * - `updateRect`: the `spatial_update_rect` action from
 *   `useSpatialFocusActions`.
 * - `layerRegistry`: optional handle to the enclosing
 *   `LayerScopeRegistry`. When provided, every freshly sampled rect is
 *   also written to its `lastKnownRect` cache so the focused-scope
 *   unmount IPC has live geometry to dispatch with even if the unmount
 *   happens between scrolls.
 *
 * # Errors
 *
 * Failures from `updateRect` are logged via `console.error` and swallowed
 * — a single dropped rect update is recoverable (the next scroll, resize,
 * or re-register call will overwrite it).
 */
export function useTrackRectOnAncestorScroll(
  nodeRef: React.RefObject<HTMLElement | null>,
  fq: FullyQualifiedMoniker,
  updateRect: UpdateRect,
  layerRegistry?: LayerScopeRegistry | null,
): void {
  useEffect(() => {
    const node = nodeRef.current;
    if (!node) return;

    let rafHandle: number | null = null;
    let cancelled = false;

    /**
     * Coalesce burst-y scroll events into one update per animation
     * frame. Reading the rect inside the rAF callback (rather than at
     * scroll-event time) keeps us aligned with the browser's paint
     * cadence and lets the engine settle on the post-scroll position
     * before we measure.
     */
    const onScroll = () => {
      if (rafHandle !== null) return;
      rafHandle = requestAnimationFrame(() => {
        rafHandle = null;
        if (cancelled) return;
        // Re-read `current` here — the effect's `node` capture is
        // still alive, but a parent re-key could have detached the
        // element from the document mid-scroll.
        const live = nodeRef.current;
        if (!live || !live.isConnected) return;
        const r = live.getBoundingClientRect();
        const rect: Rect = {
          x: asPixels(r.x),
          y: asPixels(r.y),
          width: asPixels(r.width),
          height: asPixels(r.height),
        };
        // Refresh the layer registry's cached rect alongside the
        // kernel-side update so the focused-scope-unmount IPC reads
        // post-scroll geometry rather than the mount-time sample.
        layerRegistry?.updateRect(fq, rect);
        // Capture the sample timestamp immediately after the rect read
        // so the dev-mode staleness check (`rect-validation.ts`) can
        // detect rects that age between sample and IPC dispatch. The
        // rAF-throttled scroll path is the most likely place for that
        // age to be measurable: the scroll fires, the next frame samples,
        // and a hostile schedule can stretch the queued IPC by another
        // frame.
        const sampledAtMs = performance.now();
        updateRect(fq, rect, sampledAtMs).catch((err) =>
          console.error(
            "[useTrackRectOnAncestorScroll] updateRect failed",
            err,
          ),
        );
      });
    };

    const ancestors = findScrollableAncestors(node);
    for (const ancestor of ancestors) {
      ancestor.addEventListener("scroll", onScroll, { passive: true });
    }
    // `window` covers the document-level scroller (when `<html>` /
    // `<body>` is the scrolling root, scroll events fire on `window`,
    // not on either element). Adding it unconditionally is cheap — the
    // browser only fires scroll when the document actually scrolls.
    window.addEventListener("scroll", onScroll, { passive: true });

    return () => {
      cancelled = true;
      if (rafHandle !== null) {
        cancelAnimationFrame(rafHandle);
        rafHandle = null;
      }
      for (const ancestor of ancestors) {
        ancestor.removeEventListener("scroll", onScroll);
      }
      window.removeEventListener("scroll", onScroll);
    };
  }, [nodeRef, fq, updateRect, layerRegistry]);
}
