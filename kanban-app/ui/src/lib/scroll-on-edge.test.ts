/**
 * Unit tests for `scroll-on-edge.ts`.
 *
 * Pins the contract that lets cardinal navigation cross the boundary of a
 * virtualized scroll container:
 *
 * - `scrollableAncestorInDirection(el, direction)` walks ancestors until it
 *   finds one whose computed overflow on the relevant axis is `auto` or
 *   `scroll` AND whose scroll size exceeds its client size on that axis.
 * - `canScrollFurther(el, direction)` reports whether the given scroll
 *   container has remaining travel in the requested direction.
 * - `scrollByItemHeight(el, direction, focusedRect)` advances the scroll
 *   position by one focused-item height (clamped to a sensible minimum).
 *
 * Tests run in the real-browser vitest project, so `getComputedStyle`,
 * scroll geometry, and `Element#scrollTo` behave like production.
 */

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import {
  scrollableAncestorInDirection,
  canScrollFurther,
  scrollByItemHeight,
  tryScrollOnEdge,
  runNavWithScrollOnEdge,
} from "./scroll-on-edge";
import { asFq } from "@/types/spatial";
import type {
  Direction,
  FullyQualifiedMoniker,
} from "@/types/spatial";
import type { SpatialFocusActions } from "@/lib/spatial-focus-context";

// ---------------------------------------------------------------------------
// DOM fixture helpers
// ---------------------------------------------------------------------------

/**
 * Mount an element into `document.body` and return a teardown helper. Each
 * test gets a fresh subtree so leftover scroll positions never bleed between
 * cases.
 */
function mount(node: HTMLElement): () => void {
  document.body.appendChild(node);
  return () => {
    document.body.removeChild(node);
  };
}

/**
 * Build a vertically-scrolling ancestor (`overflow-y: auto`, fixed height)
 * with a tall inner content block whose last child is the leaf the test
 * wants to navigate from. Returns the ancestor and the leaf so callers can
 * drive scroll geometry and pass the leaf to the helpers.
 */
function buildVerticalScrollFixture(opts: {
  outerHeight: number;
  innerHeight: number;
  outerOverflowY?: string;
}): {
  outer: HTMLElement;
  leaf: HTMLElement;
} {
  const outer = document.createElement("div");
  outer.style.overflowY = opts.outerOverflowY ?? "auto";
  outer.style.height = `${opts.outerHeight}px`;
  outer.style.width = "200px";

  const inner = document.createElement("div");
  inner.style.height = `${opts.innerHeight}px`;
  inner.style.width = "200px";

  const leaf = document.createElement("div");
  leaf.style.height = "40px";
  leaf.style.width = "200px";
  inner.appendChild(leaf);
  outer.appendChild(inner);
  return { outer, leaf };
}

/**
 * Build a horizontally-scrolling ancestor with a wide inner block.
 */
function buildHorizontalScrollFixture(opts: {
  outerWidth: number;
  innerWidth: number;
}): {
  outer: HTMLElement;
  leaf: HTMLElement;
} {
  const outer = document.createElement("div");
  outer.style.overflowX = "auto";
  outer.style.height = "200px";
  outer.style.width = `${opts.outerWidth}px`;

  const inner = document.createElement("div");
  inner.style.height = "200px";
  inner.style.width = `${opts.innerWidth}px`;

  const leaf = document.createElement("div");
  leaf.style.height = "40px";
  leaf.style.width = "60px";
  inner.appendChild(leaf);
  outer.appendChild(inner);
  return { outer, leaf };
}

// ---------------------------------------------------------------------------
// scrollableAncestorInDirection
// ---------------------------------------------------------------------------

describe("scrollableAncestorInDirection", () => {
  let teardown: (() => void) | null = null;

  beforeEach(() => {
    teardown = null;
  });

  afterEach(() => {
    if (teardown) teardown();
    teardown = null;
  });

  it("returns the nearest overflow-y ancestor for a vertical direction", () => {
    const { outer, leaf } = buildVerticalScrollFixture({
      outerHeight: 100,
      innerHeight: 1000,
    });
    teardown = mount(outer);

    expect(scrollableAncestorInDirection(leaf, "down")).toBe(outer);
    expect(scrollableAncestorInDirection(leaf, "up")).toBe(outer);
  });

  it("returns the nearest overflow-x ancestor for a horizontal direction", () => {
    const { outer, leaf } = buildHorizontalScrollFixture({
      outerWidth: 100,
      innerWidth: 1000,
    });
    teardown = mount(outer);

    expect(scrollableAncestorInDirection(leaf, "right")).toBe(outer);
    expect(scrollableAncestorInDirection(leaf, "left")).toBe(outer);
  });

  it("walks past `overflow: visible` ancestors", () => {
    const visibleWrap = document.createElement("div");
    visibleWrap.style.overflow = "visible";

    const { outer, leaf } = buildVerticalScrollFixture({
      outerHeight: 100,
      innerHeight: 1000,
    });
    visibleWrap.appendChild(outer);
    teardown = mount(visibleWrap);

    expect(scrollableAncestorInDirection(leaf, "down")).toBe(outer);
  });

  it("walks past `overflow: hidden` ancestors", () => {
    // `hidden` clips overflow but does not host scroll travel — the helper
    // should walk past it the same way it walks past `visible`.
    const hiddenWrap = document.createElement("div");
    hiddenWrap.style.overflow = "hidden";
    hiddenWrap.style.height = "1000px";
    hiddenWrap.style.width = "1000px";

    const { outer, leaf } = buildVerticalScrollFixture({
      outerHeight: 100,
      innerHeight: 1000,
    });
    hiddenWrap.appendChild(outer);
    teardown = mount(hiddenWrap);

    expect(scrollableAncestorInDirection(leaf, "down")).toBe(outer);
  });

  it("returns null when no ancestor scrolls on the requested axis", () => {
    const { outer, leaf } = buildHorizontalScrollFixture({
      outerWidth: 100,
      innerWidth: 1000,
    });
    teardown = mount(outer);

    // The fixture scrolls horizontally; asking for the vertical ancestor
    // must return null because none of `<body>` -> `outer` overflows on Y.
    expect(scrollableAncestorInDirection(leaf, "down")).toBeNull();
  });

  it("rejects an `overflow: auto` ancestor whose content does not exceed the client size", () => {
    // overflow-y: auto, but content fits inside the box → not scrollable.
    const { outer, leaf } = buildVerticalScrollFixture({
      outerHeight: 500,
      innerHeight: 200,
    });
    teardown = mount(outer);

    expect(scrollableAncestorInDirection(leaf, "down")).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// canScrollFurther
// ---------------------------------------------------------------------------

describe("canScrollFurther", () => {
  let teardown: (() => void) | null = null;

  afterEach(() => {
    if (teardown) teardown();
    teardown = null;
  });

  it("returns true when scrollTop is below the maximum", () => {
    const { outer } = buildVerticalScrollFixture({
      outerHeight: 100,
      innerHeight: 1000,
    });
    teardown = mount(outer);

    outer.scrollTop = 100;
    expect(canScrollFurther(outer, "down")).toBe(true);
    expect(canScrollFurther(outer, "up")).toBe(true);
  });

  it("returns false when scrollTop is at the maximum (down)", () => {
    const { outer } = buildVerticalScrollFixture({
      outerHeight: 100,
      innerHeight: 1000,
    });
    teardown = mount(outer);

    outer.scrollTop = outer.scrollHeight - outer.clientHeight;
    expect(canScrollFurther(outer, "down")).toBe(false);
    expect(canScrollFurther(outer, "up")).toBe(true);
  });

  it("returns false when scrollTop is zero (up)", () => {
    const { outer } = buildVerticalScrollFixture({
      outerHeight: 100,
      innerHeight: 1000,
    });
    teardown = mount(outer);

    outer.scrollTop = 0;
    expect(canScrollFurther(outer, "up")).toBe(false);
    expect(canScrollFurther(outer, "down")).toBe(true);
  });

  it("returns false when fully scrolled in horizontal direction", () => {
    const { outer } = buildHorizontalScrollFixture({
      outerWidth: 100,
      innerWidth: 1000,
    });
    teardown = mount(outer);

    outer.scrollLeft = outer.scrollWidth - outer.clientWidth;
    expect(canScrollFurther(outer, "right")).toBe(false);
    expect(canScrollFurther(outer, "left")).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// scrollByItemHeight
// ---------------------------------------------------------------------------

describe("scrollByItemHeight", () => {
  let teardown: (() => void) | null = null;

  afterEach(() => {
    if (teardown) teardown();
    teardown = null;
  });

  it("advances scrollTop by the focused-rect height for a vertical direction", () => {
    const { outer } = buildVerticalScrollFixture({
      outerHeight: 100,
      innerHeight: 1000,
    });
    teardown = mount(outer);

    outer.scrollTop = 0;
    scrollByItemHeight(outer, "down", { width: 100, height: 80 });
    expect(outer.scrollTop).toBe(80);
  });

  it("advances scrollLeft by the focused-rect width for a horizontal direction", () => {
    const { outer } = buildHorizontalScrollFixture({
      outerWidth: 100,
      innerWidth: 1000,
    });
    teardown = mount(outer);

    outer.scrollLeft = 0;
    scrollByItemHeight(outer, "right", { width: 120, height: 80 });
    expect(outer.scrollLeft).toBe(120);
  });

  it("uses a 64px floor when the focused rect is too small", () => {
    const { outer } = buildVerticalScrollFixture({
      outerHeight: 100,
      innerHeight: 1000,
    });
    teardown = mount(outer);

    outer.scrollTop = 0;
    scrollByItemHeight(outer, "down", { width: 100, height: 10 });
    expect(outer.scrollTop).toBe(64);
  });

  it("scrolls in the negative direction for `up`", () => {
    const { outer } = buildVerticalScrollFixture({
      outerHeight: 100,
      innerHeight: 1000,
    });
    teardown = mount(outer);

    outer.scrollTop = 200;
    scrollByItemHeight(outer, "up", { width: 100, height: 80 });
    expect(outer.scrollTop).toBe(120);
  });
});

// ---------------------------------------------------------------------------
// tryScrollOnEdge — looks up DOM by [data-moniker] and integrates with helpers
// ---------------------------------------------------------------------------

describe("tryScrollOnEdge", () => {
  let teardown: (() => void) | null = null;

  afterEach(() => {
    if (teardown) teardown();
    teardown = null;
  });

  /**
   * Mount a leaf inside a vertical scroll fixture, stamp `data-moniker` on
   * it so the scroll-on-edge lookup can find it. Returns the leaf, ancestor
   * and FQM for assertions.
   */
  function mountFocusedLeaf(opts: {
    outerHeight: number;
    innerHeight: number;
  }): {
    outer: HTMLElement;
    leaf: HTMLElement;
    fq: FullyQualifiedMoniker;
  } {
    const { outer, leaf } = buildVerticalScrollFixture(opts);
    const fq = asFq("/window/ui:board/column:c1/task:t1");
    leaf.setAttribute("data-moniker", fq);
    teardown = mount(outer);
    return { outer, leaf, fq };
  }

  it("scrolls the ancestor and returns true when there is room to scroll", () => {
    const { outer, fq } = mountFocusedLeaf({
      outerHeight: 100,
      innerHeight: 1000,
    });

    outer.scrollTop = 0;
    expect(tryScrollOnEdge(fq, "down")).toBe(true);
    expect(outer.scrollTop).toBeGreaterThan(0);
  });

  it("returns false at the bottom edge — no scroll fires", () => {
    const { outer, fq } = mountFocusedLeaf({
      outerHeight: 100,
      innerHeight: 1000,
    });

    outer.scrollTop = outer.scrollHeight - outer.clientHeight;
    const before = outer.scrollTop;
    expect(tryScrollOnEdge(fq, "down")).toBe(false);
    expect(outer.scrollTop).toBe(before);
  });

  it("returns false when the FQM is not in the DOM", () => {
    const { outer } = mountFocusedLeaf({
      outerHeight: 100,
      innerHeight: 1000,
    });
    outer.scrollTop = 0;
    const before = outer.scrollTop;
    expect(tryScrollOnEdge(asFq("/window/missing"), "down")).toBe(false);
    expect(outer.scrollTop).toBe(before);
  });
});

// ---------------------------------------------------------------------------
// runNavWithScrollOnEdge — full contract integration
// ---------------------------------------------------------------------------

describe("runNavWithScrollOnEdge", () => {
  let teardown: (() => void) | null = null;

  afterEach(() => {
    if (teardown) teardown();
    teardown = null;
  });

  /**
   * Build a stub `SpatialFocusActions` whose `focusedFq()` returns whatever
   * the test sets on `state.focused`, and whose `navigate` records every
   * dispatch into `state.calls`. The other action methods throw — the
   * scroll-on-edge harness should never call them.
   */
  function makeStubActions(state: {
    focused: FullyQualifiedMoniker | null;
    calls: Array<{ fq: FullyQualifiedMoniker; direction: Direction }>;
  }): SpatialFocusActions {
    const todo = (name: string) =>
      vi.fn(() => {
        throw new Error(`${name} should not be called by scroll-on-edge`);
      });
    return {
      registerClaim: todo("registerClaim") as unknown as SpatialFocusActions["registerClaim"],
      hasClaim: todo("hasClaim") as unknown as SpatialFocusActions["hasClaim"],
      focus: todo("focus") as unknown as SpatialFocusActions["focus"],
      clearFocus: todo("clearFocus") as unknown as SpatialFocusActions["clearFocus"],
      registerScope: todo("registerScope") as unknown as SpatialFocusActions["registerScope"],
      registerZone: todo("registerZone") as unknown as SpatialFocusActions["registerZone"],
      unregisterScope: todo(
        "unregisterScope",
      ) as unknown as SpatialFocusActions["unregisterScope"],
      updateRect: todo("updateRect") as unknown as SpatialFocusActions["updateRect"],
      navigate: vi.fn(async (fq, direction) => {
        state.calls.push({ fq, direction });
      }),
      pushLayer: todo("pushLayer") as unknown as SpatialFocusActions["pushLayer"],
      popLayer: todo("popLayer") as unknown as SpatialFocusActions["popLayer"],
      drillIn: todo("drillIn") as unknown as SpatialFocusActions["drillIn"],
      drillOut: todo("drillOut") as unknown as SpatialFocusActions["drillOut"],
      focusedFq: () => state.focused,
      subscribeFocusChanged: todo(
        "subscribeFocusChanged",
      ) as unknown as SpatialFocusActions["subscribeFocusChanged"],
    };
  }

  it("dispatches navigate exactly once when focus moves on the first try", async () => {
    const fq1 = asFq("/window/a");
    const fq2 = asFq("/window/b");
    const state = {
      focused: fq1 as FullyQualifiedMoniker | null,
      calls: [] as Array<{ fq: FullyQualifiedMoniker; direction: Direction }>,
    };
    const actions = makeStubActions(state);
    // After navigate, simulate focus changing.
    (actions.navigate as ReturnType<typeof vi.fn>).mockImplementation(
      async () => {
        state.focused = fq2;
        state.calls.push({ fq: fq1, direction: "down" });
      },
    );

    await runNavWithScrollOnEdge(actions, "down");
    expect(state.calls.length).toBe(1);
  });

  it("re-dispatches once after scrolling when the kernel returns stay-put and ancestor can scroll", async () => {
    const { outer, leaf } = buildVerticalScrollFixture({
      outerHeight: 100,
      innerHeight: 1000,
    });
    const fq = asFq("/window/leaf");
    leaf.setAttribute("data-moniker", fq);
    teardown = mount(outer);

    outer.scrollTop = 0;

    const state = {
      focused: fq as FullyQualifiedMoniker | null,
      calls: [] as Array<{ fq: FullyQualifiedMoniker; direction: Direction }>,
    };
    const actions = makeStubActions(state);
    // Both navigate calls leave focus unchanged (kernel-side stay-put,
    // simulated by not mutating `state.focused`).
    (actions.navigate as ReturnType<typeof vi.fn>).mockImplementation(
      async (from: FullyQualifiedMoniker, direction: Direction) => {
        state.calls.push({ fq: from, direction });
      },
    );

    await runNavWithScrollOnEdge(actions, "down");

    expect(state.calls.length).toBe(2);
    expect(outer.scrollTop).toBeGreaterThan(0);
  });

  it("does not retry when stay-put AND the ancestor is already at the edge", async () => {
    const { outer, leaf } = buildVerticalScrollFixture({
      outerHeight: 100,
      innerHeight: 1000,
    });
    const fq = asFq("/window/leaf");
    leaf.setAttribute("data-moniker", fq);
    teardown = mount(outer);

    outer.scrollTop = outer.scrollHeight - outer.clientHeight;
    const beforeScroll = outer.scrollTop;

    const state = {
      focused: fq as FullyQualifiedMoniker | null,
      calls: [] as Array<{ fq: FullyQualifiedMoniker; direction: Direction }>,
    };
    const actions = makeStubActions(state);
    (actions.navigate as ReturnType<typeof vi.fn>).mockImplementation(
      async (from: FullyQualifiedMoniker, direction: Direction) => {
        state.calls.push({ fq: from, direction });
      },
    );

    await runNavWithScrollOnEdge(actions, "down");

    expect(state.calls.length).toBe(1);
    expect(outer.scrollTop).toBe(beforeScroll);
  });

  it("retry depth is capped at 1 — never re-dispatches more than twice total", async () => {
    const { outer, leaf } = buildVerticalScrollFixture({
      outerHeight: 100,
      innerHeight: 1000,
    });
    const fq = asFq("/window/leaf");
    leaf.setAttribute("data-moniker", fq);
    teardown = mount(outer);

    // Mid-scroll, plenty of headroom in both directions.
    outer.scrollTop = 200;

    const state = {
      focused: fq as FullyQualifiedMoniker | null,
      calls: [] as Array<{ fq: FullyQualifiedMoniker; direction: Direction }>,
    };
    const actions = makeStubActions(state);
    (actions.navigate as ReturnType<typeof vi.fn>).mockImplementation(
      async (from: FullyQualifiedMoniker, direction: Direction) => {
        state.calls.push({ fq: from, direction });
      },
    );

    await runNavWithScrollOnEdge(actions, "down");

    // Two calls total — initial + one retry. No infinite loop.
    expect(state.calls.length).toBe(2);
  });

  it("is a no-op when nothing is focused", async () => {
    const state = {
      focused: null as FullyQualifiedMoniker | null,
      calls: [] as Array<{ fq: FullyQualifiedMoniker; direction: Direction }>,
    };
    const actions = makeStubActions(state);
    await runNavWithScrollOnEdge(actions, "down");
    expect(state.calls.length).toBe(0);
  });

  it("does not run scroll-on-edge for non-cardinal directions (`first`)", async () => {
    const { outer, leaf } = buildVerticalScrollFixture({
      outerHeight: 100,
      innerHeight: 1000,
    });
    const fq = asFq("/window/leaf");
    leaf.setAttribute("data-moniker", fq);
    teardown = mount(outer);

    outer.scrollTop = 200;
    const beforeScroll = outer.scrollTop;

    const state = {
      focused: fq as FullyQualifiedMoniker | null,
      calls: [] as Array<{ fq: FullyQualifiedMoniker; direction: Direction }>,
    };
    const actions = makeStubActions(state);
    (actions.navigate as ReturnType<typeof vi.fn>).mockImplementation(
      async (from: FullyQualifiedMoniker, direction: Direction) => {
        state.calls.push({ fq: from, direction });
      },
    );

    await runNavWithScrollOnEdge(actions, "first");

    // Only the initial navigate fires — no scroll-on-edge for jump-to-edge intents.
    expect(state.calls.length).toBe(1);
    expect(outer.scrollTop).toBe(beforeScroll);
  });
});
