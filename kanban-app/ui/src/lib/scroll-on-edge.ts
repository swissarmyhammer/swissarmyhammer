/**
 * Scroll-on-edge fall-through for the spatial-nav kernel.
 *
 * The Rust kernel is scroll-unaware: it owns the focus graph and runs
 * geometric cardinal pick over the registered scope rects, but it does not
 * know that some scopes live inside a virtualized scroll container whose
 * off-viewport rows have been unmounted (and therefore unregistered).
 *
 * When the user is on the last visible card of a virtualized column and
 * presses Down, the kernel returns stay-put — there is no registered scope
 * below the focused rect. This module is the **React-side glue** that
 * detects that case, scrolls the offending ancestor by one item-height in
 * the requested direction, and lets the caller re-dispatch nav once the
 * virtualizer has mounted the freshly-revealed row.
 *
 * The kernel itself remains scroll-unaware — see `swissarmyhammer-focus`'s
 * "Scrolling" README section.
 *
 * Three pure helpers live here:
 *
 * - {@link scrollableAncestorInDirection} — walk DOM ancestors, return the
 *   nearest one whose computed overflow + scroll geometry says it can host
 *   scroll travel on the requested axis.
 * - {@link canScrollFurther} — true when the candidate ancestor has
 *   remaining travel in the requested direction (i.e. the visual edge is
 *   not yet a true visual edge).
 * - {@link scrollByItemHeight} — advance `scrollTop` / `scrollLeft` by one
 *   item-height (or a 64px floor) in the requested direction.
 *
 * All three operate on a `Direction` literal mirroring the kernel's
 * cardinal type. `first` / `last` are not cardinal in the geometric
 * sense, so they always return `null` / no-op — the caller skips the
 * fall-through for those directions.
 */

import type { Direction, FullyQualifiedMoniker } from "@/types/spatial";
import type { SpatialFocusActions } from "@/lib/spatial-focus-context";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/**
 * Minimum scroll step. When the focused rect is smaller than this
 * (e.g. a label-only field with a 12px rect), we still need to scroll
 * far enough to bring the next virtualized row into view.
 */
const MIN_SCROLL_STEP_PX = 64;

/**
 * Tolerance used when comparing scroll positions to their bounds. Browsers
 * sometimes round subpixel scroll positions, so a strict `===` check
 * against `scrollHeight - clientHeight` produces flaky negatives near the
 * edge.
 */
const SCROLL_EDGE_EPSILON_PX = 1;

// ---------------------------------------------------------------------------
// Direction -> axis mapping
// ---------------------------------------------------------------------------

/** Axis of motion for a cardinal direction; non-cardinal directions return null. */
type Axis = "x" | "y";

/**
 * Map a cardinal `Direction` to its scroll axis. Non-cardinal values
 * (`first`, `last`) return `null` so callers can cheaply skip
 * scroll-on-edge for those.
 */
function axisFor(direction: Direction): Axis | null {
  if (direction === "up" || direction === "down") return "y";
  if (direction === "left" || direction === "right") return "x";
  return null;
}

/** Sign of motion in scrollTop / scrollLeft units for a cardinal direction. */
function signFor(direction: Direction): 1 | -1 | 0 {
  if (direction === "down" || direction === "right") return 1;
  if (direction === "up" || direction === "left") return -1;
  return 0;
}

// ---------------------------------------------------------------------------
// scrollableAncestorInDirection
// ---------------------------------------------------------------------------

/**
 * Walk the DOM ancestor chain of `el` and return the nearest element that
 * can host scroll travel on the axis matching `direction`.
 *
 * An element qualifies when:
 *
 * 1. Its computed `overflow-y` (vertical) or `overflow-x` (horizontal) is
 *    `auto` or `scroll`. `visible` and `hidden` ancestors are
 *    walked through.
 * 2. The element's scroll size on the axis exceeds its client size — i.e.
 *    the user can actually scroll it. An ancestor whose overflow is `auto`
 *    but whose content fits inside the box is not a real scroll container
 *    and gets walked past.
 *
 * Returns `null` for non-cardinal directions or when no ancestor qualifies.
 *
 * @param el       Starting node — typically the focused scope's DOM node.
 * @param direction Cardinal direction the caller is trying to navigate.
 */
export function scrollableAncestorInDirection(
  el: Element,
  direction: Direction,
): HTMLElement | null {
  const axis = axisFor(direction);
  if (axis === null) return null;

  let parent = el.parentElement;
  while (parent) {
    if (isScrollableOnAxis(parent, axis)) return parent;
    parent = parent.parentElement;
  }
  return null;
}

/**
 * Return true when `el`'s computed overflow on `axis` is scrollable AND
 * the element's content overflows that axis.
 *
 * Only `auto` and `scroll` count as scrollable. The legacy `overlay` value
 * (Webkit-only, deprecated by the CSS Overflow spec) is intentionally not
 * accepted — modern browsers normalize it to `auto`, so accepting it here
 * would only matter on obsolete engines.
 */
function isScrollableOnAxis(el: HTMLElement, axis: Axis): boolean {
  const style = window.getComputedStyle(el);
  const overflow = axis === "y" ? style.overflowY : style.overflowX;
  if (overflow !== "auto" && overflow !== "scroll") {
    return false;
  }
  const scrollSize = axis === "y" ? el.scrollHeight : el.scrollWidth;
  const clientSize = axis === "y" ? el.clientHeight : el.clientWidth;
  return scrollSize > clientSize;
}

// ---------------------------------------------------------------------------
// canScrollFurther
// ---------------------------------------------------------------------------

/**
 * Return true when `el` has remaining scroll travel in `direction`.
 *
 * The check uses a 1 px epsilon to absorb the subpixel rounding browsers
 * apply at scroll bounds.
 *
 * Non-cardinal directions return `false` — the caller should not invoke
 * scroll-on-edge for those.
 */
export function canScrollFurther(
  el: HTMLElement,
  direction: Direction,
): boolean {
  const axis = axisFor(direction);
  if (axis === null) return false;

  if (axis === "y") {
    const max = el.scrollHeight - el.clientHeight;
    if (direction === "down") return el.scrollTop < max - SCROLL_EDGE_EPSILON_PX;
    return el.scrollTop > SCROLL_EDGE_EPSILON_PX;
  }
  const max = el.scrollWidth - el.clientWidth;
  if (direction === "right") return el.scrollLeft < max - SCROLL_EDGE_EPSILON_PX;
  return el.scrollLeft > SCROLL_EDGE_EPSILON_PX;
}

// ---------------------------------------------------------------------------
// scrollByItemHeight
// ---------------------------------------------------------------------------

/** Subset of a `DOMRect` that the scroll step needs. */
export interface FocusedRect {
  width: number;
  height: number;
}

/**
 * Advance `el`'s scroll position by one item-height in `direction`.
 *
 * The step is `max(focusedRect.height|width, MIN_SCROLL_STEP_PX)` so a
 * narrow leaf (e.g. a 12 px label) still scrolls far enough to mount the
 * next virtualized row.
 *
 * No-op for non-cardinal directions.
 */
export function scrollByItemHeight(
  el: HTMLElement,
  direction: Direction,
  focusedRect: FocusedRect,
): void {
  const axis = axisFor(direction);
  if (axis === null) return;

  const sign = signFor(direction);
  if (sign === 0) return;

  const itemSize = axis === "y" ? focusedRect.height : focusedRect.width;
  const step = Math.max(itemSize, MIN_SCROLL_STEP_PX);
  if (axis === "y") {
    el.scrollTop = el.scrollTop + sign * step;
  } else {
    el.scrollLeft = el.scrollLeft + sign * step;
  }
}

// ---------------------------------------------------------------------------
// runNavWithScrollOnEdge — the wired contract
// ---------------------------------------------------------------------------

/**
 * Cardinal directions get the scroll-on-edge fall-through; jump-to-edge
 * directions (`first`, `last`) do not.
 */
function isCardinal(direction: Direction): boolean {
  return (
    direction === "up" ||
    direction === "down" ||
    direction === "left" ||
    direction === "right"
  );
}

/**
 * Wait for one animation frame.
 *
 * Used between `await actions.navigate(...)` and the follow-on
 * `focusedFq()` read so the asynchronously-delivered `focus-changed`
 * event has had a chance to land in the `<SpatialFocusProvider>`'s
 * focused-FQM ref. Also gives a freshly-scrolled virtualizer a
 * measurement frame to mount the newly-revealed row before the retry
 * `navigate` fires.
 */
function nextFrame(): Promise<void> {
  return new Promise((resolve) => requestAnimationFrame(() => resolve()));
}

/**
 * Look up the DOM node owned by `<FocusScope>` / `<FocusZone>` for the
 * given FQM. Both primitives stamp `data-moniker={fq}` on their root
 * div; reading by that attribute is the React-side mirror of the
 * kernel's registry lookup.
 *
 * Returns `null` when the focused scope is not in the DOM (e.g. a
 * virtualizer just unmounted it, or the test harness mounted the
 * primitives outside `document.body`).
 */
function findFocusedNode(fq: FullyQualifiedMoniker): HTMLElement | null {
  const escaped =
    typeof CSS !== "undefined" && typeof CSS.escape === "function"
      ? CSS.escape(fq)
      : (fq as string).replace(/(["\\])/g, "\\$1");
  return document.querySelector(
    `[data-moniker="${escaped}"]`,
  ) as HTMLElement | null;
}

/**
 * Find the focused scope's nearest scrollable ancestor in `direction`,
 * scroll it by one item-height, and report whether the scroll fired.
 *
 * Returns `false` when the focused scope is not in the DOM, has no
 * scrollable ancestor on the requested axis, or that ancestor is
 * already at the visual edge in the requested direction (no remaining
 * travel). Returns `true` when scroll mutation occurred — the caller
 * should await a frame for the virtualizer's measurement cycle and
 * re-dispatch nav.
 */
export function tryScrollOnEdge(
  fq: FullyQualifiedMoniker,
  direction: Direction,
): boolean {
  const node = findFocusedNode(fq);
  if (node === null) return false;
  const ancestor = scrollableAncestorInDirection(node, direction);
  if (ancestor === null) return false;
  if (!canScrollFurther(ancestor, direction)) return false;
  const rect = node.getBoundingClientRect();
  scrollByItemHeight(ancestor, direction, {
    width: rect.width,
    height: rect.height,
  });
  return true;
}

/**
 * Run cardinal nav with the scroll-on-edge fall-through.
 *
 * Contract:
 *
 * 1. Read the focused FQM, dispatch `actions.navigate(fq, direction)`.
 * 2. For non-cardinal directions, return — scroll-on-edge does not
 *    apply to jump-to-edge intents (`first`, `last`).
 * 3. Wait one animation frame so the kernel's `focus-changed` event
 *    can land in the spatial-focus provider's ref.
 * 4. If `focusedFq()` changed, focus moved — return.
 * 5. Otherwise the kernel returned stay-put. Try
 *    {@link tryScrollOnEdge}. If no scroll fired (no ancestor or true
 *    visual edge), return — the user has hit a real edge.
 * 6. Wait one animation frame for the virtualizer to mount the
 *    newly-revealed row, then re-read `focusedFq()` and re-dispatch
 *    nav exactly once. The retry depth is capped at 1 so a weird
 *    layout cannot produce an infinite loop.
 *
 * No-op when nothing is focused (`focusedFq() === null`) — there is
 * nothing to navigate from.
 *
 * # Why we re-read `focusedFq()` before the retry navigate
 *
 * The first `await nextFrame()` (step 3) is the obvious race window —
 * an asynchronous focus-changed event could land while we wait, and
 * step 4's guard handles that. The second `await nextFrame()` (step 6)
 * is a subtler window: while we wait for the virtualizer's measurement
 * cycle, *another* focus-changed event could land (e.g. an unrelated
 * pointer click, a programmatic `actions.focus()` from another part of
 * the app). Re-reading `focusedFq()` before the retry ensures we
 * dispatch from the *current* focused FQM, not the one we captured
 * before scrolling. The kernel handles unknown / stale FQMs gracefully
 * by returning stay-put, so this is defense-in-depth rather than a
 * correctness fix — but issuing nav from a stale FQM risks one wasted
 * IPC and a confusing trace, both of which the re-read avoids.
 *
 * Lives in React glue, not the Rust kernel: the kernel is
 * scroll-unaware (it only sees rects of registered scopes, not the
 * DOM scroll containers that gate which scopes are even mounted).
 * See `swissarmyhammer-focus/README.md` "Scrolling" section.
 */
export async function runNavWithScrollOnEdge(
  actions: SpatialFocusActions,
  direction: Direction,
): Promise<void> {
  const fq = actions.focusedFq();
  if (fq === null) return;
  await actions.navigate(fq, direction);

  if (!isCardinal(direction)) return;

  await nextFrame();
  if (actions.focusedFq() !== fq) return;

  if (!tryScrollOnEdge(fq, direction)) return;

  await nextFrame();
  // Re-read the focused FQM after the post-scroll frame: a focus-changed
  // event from another part of the app could have landed during the
  // measurement-cycle wait. See the docstring's "Why we re-read" note.
  const fqAfterScroll = actions.focusedFq();
  if (fqAfterScroll === null || fqAfterScroll !== fq) return;
  await actions.navigate(fqAfterScroll, direction);
}
