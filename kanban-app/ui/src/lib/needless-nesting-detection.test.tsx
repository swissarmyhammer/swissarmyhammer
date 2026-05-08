/**
 * Tests for the dev-mode needless-nesting detection that hooks into
 * `LayerScopeRegistry.add` and warns when two scopes share a rect.
 *
 * The detection is the JS replacement for the now-deleted Rust
 * `check_overlap_warning`. It runs only on registry insertion (not on
 * every rect update), which is why drag-drop animations no longer
 * trigger spurious warnings while structural needless-nesting still
 * does.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { type RefObject } from "react";

import {
  LayerScopeRegistry,
  __test__,
  type ScopeEntry,
} from "./layer-scope-registry-context";

// Bind the test-only helpers locally so the bulk of each test reads the
// same way it did when these were direct named exports. The `__test__`
// indirection exists only so tree-shaking can drop the helpers from
// production bundles (see the doc-comment on the export).
const { detectNeedlessNesting, rectsOverlapTightly } = __test__;
import {
  asSegment,
  composeFq,
  fqRoot,
  type FullyQualifiedMoniker,
  type SegmentMoniker,
} from "@/types/spatial";

/* -------------------------------------------------------------------------- */
/* Helpers                                                                    */
/* -------------------------------------------------------------------------- */

interface DomRectShape {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Build a `ScopeEntry` whose `ref.current` is a real DOM node with a
 * stubbed `getBoundingClientRect`. Returns the entry so the caller can
 * attach it to a registry, plus a `setRect` setter so the test can move
 * the node mid-test (simulating drag) without re-mounting. */
function makeEntryWithRect(
  rect: DomRectShape,
  segment: SegmentMoniker = asSegment("scope"),
  parentZone: FullyQualifiedMoniker | null = null,
): {
  entry: ScopeEntry;
  setRect: (next: DomRectShape) => void;
} {
  const node = document.createElement("div");
  let current = rect;
  node.getBoundingClientRect = () =>
    ({
      x: current.x,
      y: current.y,
      width: current.width,
      height: current.height,
      top: current.y,
      left: current.x,
      right: current.x + current.width,
      bottom: current.y + current.height,
      toJSON: () => current,
    }) as DOMRect;
  const ref: RefObject<HTMLElement | null> = { current: node };
  return {
    entry: {
      ref,
      parentZone,
      segment,
      lastKnownRect: null,
    },
    setRect: (next) => {
      current = next;
    },
  };
}

/** Microtask flush so the `queueMicrotask` body inside `add()` runs. */
async function flushMicrotasks() {
  await Promise.resolve();
}

const layerFq = fqRoot(asSegment("window"));

/* -------------------------------------------------------------------------- */
/* `rectsOverlapTightly` — pure helper                                        */
/* -------------------------------------------------------------------------- */

describe("rectsOverlapTightly", () => {
  it("returns true when both rects are identical", () => {
    const r = { x: 10, y: 20, width: 100, height: 50 };
    expect(rectsOverlapTightly(r, r)).toBe(true);
  });

  it("returns true when all four sides agree within the default 2 px tolerance", () => {
    const a = { x: 10, y: 20, width: 100, height: 50 };
    // All four sides shift by < 2 px:
    //   x:      11.4 vs 10    → diff 1.4
    //   y:      19.2 vs 20    → diff 0.8
    //   right:  111.0 vs 110  → diff 1.0
    //   bottom: 70.7 vs 70    → diff 0.7
    const b = { x: 11.4, y: 19.2, width: 99.6, height: 51.5 };
    expect(rectsOverlapTightly(a, b)).toBe(true);
  });

  it("returns false when origin diverges beyond tolerance", () => {
    const a = { x: 10, y: 20, width: 100, height: 50 };
    const b = { x: 50, y: 20, width: 100, height: 50 };
    expect(rectsOverlapTightly(a, b)).toBe(false);
  });

  it("returns false when right edge diverges beyond tolerance", () => {
    const a = { x: 10, y: 20, width: 100, height: 50 };
    const b = { x: 10, y: 20, width: 200, height: 50 };
    expect(rectsOverlapTightly(a, b)).toBe(false);
  });

  it("returns false when bottom edge diverges beyond tolerance", () => {
    const a = { x: 10, y: 20, width: 100, height: 50 };
    const b = { x: 10, y: 20, width: 100, height: 200 };
    expect(rectsOverlapTightly(a, b)).toBe(false);
  });

  it("respects a custom tolerance argument", () => {
    const a = { x: 10, y: 20, width: 100, height: 50 };
    const b = { x: 13, y: 20, width: 100, height: 50 };
    expect(rectsOverlapTightly(a, b, 2)).toBe(false);
    expect(rectsOverlapTightly(a, b, 4)).toBe(true);
  });
});

/* -------------------------------------------------------------------------- */
/* `detectNeedlessNesting` — direct (synchronous) invocation                  */
/* -------------------------------------------------------------------------- */

describe("detectNeedlessNesting (direct)", () => {
  let warnSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
  });

  afterEach(() => {
    warnSpy.mockRestore();
  });

  it("emits one structured warning per overlapping partner", () => {
    const reg = new LayerScopeRegistry(layerFq);
    const aFq = composeFq(layerFq, asSegment("a"));
    const bFq = composeFq(layerFq, asSegment("b"));

    const a = makeEntryWithRect(
      { x: 10, y: 20, width: 100, height: 50 },
      asSegment("a"),
    );
    const b = makeEntryWithRect(
      { x: 10, y: 20, width: 100, height: 50 },
      asSegment("b"),
    );

    reg.add(aFq, a.entry);
    reg.add(bFq, b.entry);

    // Direct (synchronous) call — bypasses the microtask scheduling so
    // the test can assert on the structured payload deterministically.
    detectNeedlessNesting(bFq, b.entry, reg);

    expect(warnSpy).toHaveBeenCalledTimes(1);
    const [message, payload] = warnSpy.mock.calls[0] as [
      string,
      Record<string, unknown>,
    ];
    expect(message).toBe(
      "[spatial-nav] needless-nesting: two scopes share rect",
    );
    expect(payload).toMatchObject({
      newFq: bFq,
      otherFq: aFq,
      newSegment: asSegment("b"),
      otherSegment: asSegment("a"),
      rect: { x: 10, y: 20, width: 100, height: 50 },
    });
  });

  it("does not warn when rects diverge (drag-style position change)", () => {
    const reg = new LayerScopeRegistry(layerFq);
    const aFq = composeFq(layerFq, asSegment("a"));
    const bFq = composeFq(layerFq, asSegment("b"));

    const a = makeEntryWithRect({ x: 10, y: 20, width: 100, height: 50 });
    const b = makeEntryWithRect({ x: 200, y: 400, width: 100, height: 50 });

    reg.add(aFq, a.entry);
    reg.add(bFq, b.entry);

    detectNeedlessNesting(bFq, b.entry, reg);
    expect(warnSpy).not.toHaveBeenCalled();
  });

  it("skips the new entry when its node is detached", () => {
    const reg = new LayerScopeRegistry(layerFq);
    const aFq = composeFq(layerFq, asSegment("a"));
    const bFq = composeFq(layerFq, asSegment("b"));

    const a = makeEntryWithRect({ x: 10, y: 20, width: 100, height: 50 });
    const detached: ScopeEntry = {
      ref: { current: null },
      parentZone: null,
      segment: asSegment("b"),
      lastKnownRect: null,
    };

    reg.add(aFq, a.entry);
    reg.add(bFq, detached);

    expect(() => detectNeedlessNesting(bFq, detached, reg)).not.toThrow();
    expect(warnSpy).not.toHaveBeenCalled();
  });

  it("skips an other-entry whose node has gone null mid-iteration", () => {
    const reg = new LayerScopeRegistry(layerFq);
    const aFq = composeFq(layerFq, asSegment("a"));
    const bFq = composeFq(layerFq, asSegment("b"));

    const a = makeEntryWithRect({ x: 10, y: 20, width: 100, height: 50 });
    const b = makeEntryWithRect({ x: 10, y: 20, width: 100, height: 50 });

    reg.add(aFq, a.entry);
    reg.add(bFq, b.entry);

    // Simulate the brief window between React scheduling unmount and
    // the cleanup running by nulling the partner's ref before the
    // detection runs.
    a.entry.ref.current = null;

    detectNeedlessNesting(bFq, b.entry, reg);
    expect(warnSpy).not.toHaveBeenCalled();
  });

  it("skips zero-dimension rects (pre-layout / display:none artefacts)", () => {
    const reg = new LayerScopeRegistry(layerFq);
    const aFq = composeFq(layerFq, asSegment("a"));
    const bFq = composeFq(layerFq, asSegment("b"));

    const a = makeEntryWithRect({ x: 0, y: 0, width: 0, height: 0 });
    const b = makeEntryWithRect({ x: 0, y: 0, width: 0, height: 0 });

    reg.add(aFq, a.entry);
    reg.add(bFq, b.entry);

    detectNeedlessNesting(bFq, b.entry, reg);
    expect(warnSpy).not.toHaveBeenCalled();
  });
});

/* -------------------------------------------------------------------------- */
/* `add()` hook timing — microtask, not the next macrotask                    */
/* -------------------------------------------------------------------------- */

describe("LayerScopeRegistry.add — needless-nesting hook timing", () => {
  let warnSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
  });

  afterEach(() => {
    warnSpy.mockRestore();
  });

  // The `add()` hook is gated on a literal `import.meta.env.DEV`.
  // Vitest's default test environment sets DEV=true, so `add()` actually
  // queues the microtask. Each test here relies on that default and
  // asserts the timing accordingly.

  it("requires DEV=true at runtime in this test environment", () => {
    expect(import.meta.env.DEV).toBe(true);
  });

  it("warns after a microtask flush when two adds share a rect", async () => {
    const reg = new LayerScopeRegistry(layerFq);
    const aFq = composeFq(layerFq, asSegment("a"));
    const bFq = composeFq(layerFq, asSegment("b"));

    const a = makeEntryWithRect({ x: 10, y: 20, width: 100, height: 50 });
    const b = makeEntryWithRect({ x: 10, y: 20, width: 100, height: 50 });

    reg.add(aFq, a.entry);
    reg.add(bFq, b.entry);

    // Synchronously: nothing has fired yet — the microtask is queued.
    expect(warnSpy).not.toHaveBeenCalled();

    await flushMicrotasks();

    // After the microtask flush: both queued detections have run. The
    // first (`a`) saw an empty registry, the second (`b`) saw `a` and
    // warned.
    expect(warnSpy).toHaveBeenCalledTimes(1);
    const [, payload] = warnSpy.mock.calls[0] as [
      string,
      Record<string, unknown>,
    ];
    expect(payload).toMatchObject({ newFq: bFq, otherFq: aFq });
  });

  it("no warning when the second scope has a different rect (drag-style)", async () => {
    const reg = new LayerScopeRegistry(layerFq);
    const aFq = composeFq(layerFq, asSegment("a"));
    const bFq = composeFq(layerFq, asSegment("b"));

    const a = makeEntryWithRect({ x: 10, y: 20, width: 100, height: 50 });
    const b = makeEntryWithRect({ x: 200, y: 400, width: 100, height: 50 });

    reg.add(aFq, a.entry);
    reg.add(bFq, b.entry);
    await flushMicrotasks();

    expect(warnSpy).not.toHaveBeenCalled();
  });

  it("rect changes after registration do NOT trigger the warning (drag scenario)", async () => {
    const reg = new LayerScopeRegistry(layerFq);
    const aFq = composeFq(layerFq, asSegment("a"));

    const a = makeEntryWithRect({ x: 10, y: 20, width: 100, height: 50 });
    reg.add(aFq, a.entry);
    await flushMicrotasks();
    expect(warnSpy).not.toHaveBeenCalled();

    // Move the card across the column — the rect changes but no new
    // scope mounts. The detection runs only on `add`, so no warning.
    a.setRect({ x: 500, y: 600, width: 100, height: 50 });
    reg.updateRect(aFq, {
      x: 500 as never,
      y: 600 as never,
      width: 100 as never,
      height: 50 as never,
    });
    await flushMicrotasks();

    expect(warnSpy).not.toHaveBeenCalled();
  });

  it("filter unmount + remount at a different position does NOT warn", async () => {
    const reg = new LayerScopeRegistry(layerFq);
    const aFq = composeFq(layerFq, asSegment("a"));
    const bFq = composeFq(layerFq, asSegment("b"));

    const a = makeEntryWithRect({ x: 10, y: 20, width: 100, height: 50 });
    reg.add(aFq, a.entry);
    await flushMicrotasks();

    reg.delete(aFq);
    const b = makeEntryWithRect({ x: 500, y: 600, width: 100, height: 50 });
    reg.add(bFq, b.entry);
    await flushMicrotasks();

    expect(warnSpy).not.toHaveBeenCalled();
  });

  it("filter unmount + remount at the SAME position DOES warn", async () => {
    const reg = new LayerScopeRegistry(layerFq);
    const aFq = composeFq(layerFq, asSegment("a"));
    const bFq = composeFq(layerFq, asSegment("b"));

    const a = makeEntryWithRect({ x: 10, y: 20, width: 100, height: 50 });
    reg.add(aFq, a.entry);
    await flushMicrotasks();
    expect(warnSpy).not.toHaveBeenCalled();

    // Different FQ, but a new card replaces the prior occupant at the
    // same on-screen rect — the structural pattern this detection exists
    // to catch (the inner `<FocusScope>` of a single-child wrapper).
    reg.delete(aFq);
    const b = makeEntryWithRect({ x: 10, y: 20, width: 100, height: 50 });
    reg.add(bFq, b.entry);
    await flushMicrotasks();

    // No partner is left for the inner one to share with — `a` is gone.
    expect(warnSpy).not.toHaveBeenCalled();

    // Now mount a sibling that genuinely overlaps the surviving `b`.
    const cFq = composeFq(layerFq, asSegment("c"));
    const c = makeEntryWithRect({ x: 10, y: 20, width: 100, height: 50 });
    reg.add(cFq, c.entry);
    await flushMicrotasks();

    expect(warnSpy).toHaveBeenCalledTimes(1);
    const [, payload] = warnSpy.mock.calls[0] as [
      string,
      Record<string, unknown>,
    ];
    expect(payload).toMatchObject({ newFq: cFq, otherFq: bFq });
  });

  it("the literal needless-nesting pattern (parent + sole child at same rect) warns", async () => {
    const reg = new LayerScopeRegistry(layerFq);
    const parentFq = composeFq(layerFq, asSegment("parent"));
    const childFq = composeFq(parentFq, asSegment("child"));

    const parentRect = { x: 0, y: 0, width: 200, height: 100 };
    const parent = makeEntryWithRect(parentRect, asSegment("parent"));
    const child = makeEntryWithRect(
      // Identical rect — child fills its parent with no offset/padding.
      parentRect,
      asSegment("child"),
      parentFq,
    );

    reg.add(parentFq, parent.entry);
    reg.add(childFq, child.entry);
    await flushMicrotasks();

    expect(warnSpy).toHaveBeenCalledTimes(1);
    const [, payload] = warnSpy.mock.calls[0] as [
      string,
      Record<string, unknown>,
    ];
    expect(payload).toMatchObject({
      newFq: childFq,
      otherFq: parentFq,
      newSegment: asSegment("child"),
      otherSegment: asSegment("parent"),
    });
  });

  it("detector exceptions are caught and logged, never thrown out of add()", async () => {
    const reg = new LayerScopeRegistry(layerFq);
    const aFq = composeFq(layerFq, asSegment("a"));
    const a = makeEntryWithRect({ x: 0, y: 0, width: 10, height: 10 });

    // Sabotage the entry's ref so reading `getBoundingClientRect`
    // throws. The microtask body must catch it.
    Object.defineProperty(a.entry.ref, "current", {
      get() {
        throw new Error("sabotage");
      },
    });

    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    expect(() => reg.add(aFq, a.entry)).not.toThrow();
    await flushMicrotasks();

    expect(errorSpy).toHaveBeenCalledWith(
      "[LayerScopeRegistry] needless-nesting detector threw",
      expect.any(Error),
    );
    errorSpy.mockRestore();
  });
});

/* -------------------------------------------------------------------------- */
/* Production-build smoke — DEV=false short-circuits the hook                 */
/* -------------------------------------------------------------------------- */

describe("LayerScopeRegistry.add — production builds carry no detection cost", () => {
  let warnSpy: ReturnType<typeof vi.spyOn>;
  let originalDev: unknown;

  beforeEach(() => {
    warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    // Direct mutation of `import.meta.env.DEV` is the most reliable
    // way to flip the gate in vitest browser-mode runs (`vi.stubEnv`
    // does not always plumb DEV through to user code).
    originalDev = import.meta.env.DEV;
    (import.meta.env as Record<string, unknown>).DEV = false;
  });

  afterEach(() => {
    (import.meta.env as Record<string, unknown>).DEV = originalDev;
    warnSpy.mockRestore();
  });

  it("does not queue the microtask when DEV is false", async () => {
    expect(import.meta.env.DEV).toBe(false);

    const reg = new LayerScopeRegistry(layerFq);
    const aFq = composeFq(layerFq, asSegment("a"));
    const bFq = composeFq(layerFq, asSegment("b"));

    const a = makeEntryWithRect({ x: 10, y: 20, width: 100, height: 50 });
    const b = makeEntryWithRect({ x: 10, y: 20, width: 100, height: 50 });

    reg.add(aFq, a.entry);
    reg.add(bFq, b.entry);
    await flushMicrotasks();

    expect(warnSpy).not.toHaveBeenCalled();
  });
});
