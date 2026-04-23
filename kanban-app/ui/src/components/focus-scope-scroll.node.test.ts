/**
 * Unit tests for the scroll-ancestor walk used by `useRectObserver`.
 *
 * ## Why this test exists
 *
 * Board columns use `@tanstack/react-virtual`, which positions cards with
 * `transform: translateY(px)`. `ResizeObserver` does not fire on transform
 * changes, so cards' rects in the Rust spatial state stay stuck at their
 * first-measured coordinates as the column scrolls. The fix: when the
 * nearest scrollable ancestor of a spatial-registered element scrolls,
 * re-report the element's rect.
 *
 * The pure piece this file exercises is `findScrollableAncestor()` —
 * walk up the DOM from a given element and return the first ancestor
 * whose computed `overflow`/`overflowY`/`overflowX` is `auto`, `scroll`,
 * or `overlay`. If none is found, return `null`. The scroll listener is
 * wired in the React hook (`useRectObserver`); this file only locks in
 * the ancestor-detection contract.
 *
 * ## Why node + happy-dom
 *
 * Walking up the DOM and querying `getComputedStyle` is the exact
 * surface happy-dom provides. Running the test in the node project
 * keeps the fast feedback loop — no Chromium spin-up for a pure DOM
 * walk.
 */

import { describe, it, expect, beforeEach } from "vitest";
import { findScrollableAncestor } from "@/components/focus-scope";

describe("findScrollableAncestor", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
  });

  it("returns the nearest ancestor with overflow: auto", () => {
    const outer = document.createElement("div");
    const scroller = document.createElement("div");
    scroller.style.overflow = "auto";
    const inner = document.createElement("div");
    outer.appendChild(scroller);
    scroller.appendChild(inner);
    document.body.appendChild(outer);

    expect(findScrollableAncestor(inner)).toBe(scroller);
  });

  it("returns the nearest ancestor with overflow: scroll", () => {
    const scroller = document.createElement("div");
    scroller.style.overflow = "scroll";
    const inner = document.createElement("div");
    scroller.appendChild(inner);
    document.body.appendChild(scroller);

    expect(findScrollableAncestor(inner)).toBe(scroller);
  });

  it("returns the nearest ancestor with overflow-y: auto even when overflow-x is visible", () => {
    const scroller = document.createElement("div");
    scroller.style.overflowY = "auto";
    scroller.style.overflowX = "visible";
    const inner = document.createElement("div");
    scroller.appendChild(inner);
    document.body.appendChild(scroller);

    expect(findScrollableAncestor(inner)).toBe(scroller);
  });

  it("returns the nearest ancestor with overflow-x: scroll", () => {
    const scroller = document.createElement("div");
    scroller.style.overflowX = "scroll";
    const inner = document.createElement("div");
    scroller.appendChild(inner);
    document.body.appendChild(scroller);

    expect(findScrollableAncestor(inner)).toBe(scroller);
  });

  it("skips non-scrollable ancestors and returns the first scrollable one", () => {
    const scroller = document.createElement("div");
    scroller.style.overflow = "auto";
    const middle = document.createElement("div");
    // middle has no overflow style — should be skipped.
    const inner = document.createElement("div");
    scroller.appendChild(middle);
    middle.appendChild(inner);
    document.body.appendChild(scroller);

    expect(findScrollableAncestor(inner)).toBe(scroller);
  });

  it("returns null when no ancestor is scrollable", () => {
    const outer = document.createElement("div");
    const inner = document.createElement("div");
    outer.appendChild(inner);
    document.body.appendChild(outer);

    expect(findScrollableAncestor(inner)).toBeNull();
  });

  it("ignores overflow: visible", () => {
    const outer = document.createElement("div");
    outer.style.overflow = "visible";
    const inner = document.createElement("div");
    outer.appendChild(inner);
    document.body.appendChild(outer);

    expect(findScrollableAncestor(inner)).toBeNull();
  });

  it("ignores overflow: hidden", () => {
    // `hidden` does not scroll — the viewport cannot be moved by the
    // user, so a scroll event on a `hidden`-overflow parent cannot
    // change the rect of its children.
    const outer = document.createElement("div");
    outer.style.overflow = "hidden";
    const inner = document.createElement("div");
    outer.appendChild(inner);
    document.body.appendChild(outer);

    expect(findScrollableAncestor(inner)).toBeNull();
  });

  it("does not return the element itself", () => {
    // Even when the element itself is the scroller, the caller wants
    // the ancestor that moves relative to the viewport — which is the
    // parent scroll context, not the element being measured.
    const outer = document.createElement("div");
    outer.style.overflow = "auto";
    const self = document.createElement("div");
    self.style.overflow = "auto";
    outer.appendChild(self);
    document.body.appendChild(outer);

    expect(findScrollableAncestor(self)).toBe(outer);
  });
});
