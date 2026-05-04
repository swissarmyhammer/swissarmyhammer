/**
 * Unit tests for `rect-validation.ts`.
 *
 * Pins the four invariants the dev-mode validator enforces:
 *
 *   1. A valid rect produces no errors and no warnings.
 *   2. A non-finite component produces exactly one error per offender.
 *   3. A non-positive width or height produces an error.
 *   4. Coordinates outside the plausible viewport range produce an error.
 *   5. A stale sample timestamp produces a warning.
 *
 * The validator is **observability-only** — it never throws and never
 * blocks the IPC. Tests assert on returned `errors` / `warnings` arrays
 * and on `console.error` / `console.warn` spies attached via Vitest.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  validateRect,
  validateAndLogRect,
  isDevModeRectValidationEnabled,
  __resetPreLayoutTransientLog,
} from "./rect-validation";
import { asPixels, asFq } from "@/types/spatial";
import type { Rect } from "@/types/spatial";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Build a `Rect` from raw numbers. The branded `Pixels` type makes this
 * verbose at the test callsite; the helper hides the noise.
 */
function rect(x: number, y: number, width: number, height: number): Rect {
  return {
    x: asPixels(x),
    y: asPixels(y),
    width: asPixels(width),
    height: asPixels(height),
  };
}

/**
 * A plausible viewport-relative rect: somewhere in a 1920x1080 desktop
 * area, both dimensions positive, all values finite.
 */
function plausibleRect(): Rect {
  return rect(120, 80, 240, 40);
}

// ---------------------------------------------------------------------------
// Pure validator
// ---------------------------------------------------------------------------

describe("validateRect", () => {
  it("passes a plausible viewport-relative rect on register_scope", () => {
    const result = validateRect("register_scope", plausibleRect(), 100, 105);
    expect(result.errors).toEqual([]);
    expect(result.warnings).toEqual([]);
    expect(result.preLayoutTransient).toBe(false);
  });

  it("flags negative width as an error", () => {
    const result = validateRect(
      "register_scope",
      rect(0, 0, -10, 40),
      100,
      105,
    );
    expect(result.errors).toHaveLength(1);
    expect(result.errors[0]).toMatch(/width must be > 0/);
    expect(result.preLayoutTransient).toBe(false);
  });

  it("flags negative height as an error", () => {
    const result = validateRect(
      "register_scope",
      rect(0, 0, 40, -1),
      100,
      105,
    );
    expect(result.errors).toHaveLength(1);
    expect(result.errors[0]).toMatch(/height must be > 0/);
    expect(result.preLayoutTransient).toBe(false);
  });

  it("treats zero-dim rect on register_scope as a pre-layout transient warning", () => {
    // On `register_scope`, a zero in either dimension is the structural
    // shape `getBoundingClientRect()` produces for `display: none`,
    // just-mounted-but-not-yet-laid-out, and detached nodes. Surface
    // as a warning, not an error, so the error channel stays clean.
    const result = validateRect(
      "register_scope",
      rect(0, 0, 100, 0),
      100,
      105,
    );
    expect(result.errors).toEqual([]);
    expect(result.warnings.length).toBeGreaterThanOrEqual(1);
    expect(result.warnings[0]).toMatch(/zero dimension/);
    expect(result.preLayoutTransient).toBe(true);
  });

  it("treats both-zero rect on register_scope as a pre-layout transient warning", () => {
    const result = validateRect("register_scope", rect(0, 0, 0, 0), 100, 105);
    expect(result.errors).toEqual([]);
    expect(result.warnings.length).toBeGreaterThanOrEqual(1);
    expect(result.warnings[0]).toMatch(/zero dimension/);
    expect(result.preLayoutTransient).toBe(true);
  });

  it("flags zero-dim rect on update_rect as a real error", () => {
    // `update_rect` runs from ResizeObserver / ancestor-scroll listener,
    // both of which fire only after layout. A zero dim at this point is
    // a real bug — the kernel will record a persistent broken rect.
    const result = validateRect("update_rect", rect(0, 0, 100, 0), 100, 105);
    expect(result.errors).toHaveLength(1);
    expect(result.errors[0]).toMatch(/height must be > 0/);
    expect(result.preLayoutTransient).toBe(false);
  });

  it("flags both-zero rect on update_rect as two errors", () => {
    const result = validateRect("update_rect", rect(0, 0, 0, 0), 100, 105);
    expect(result.errors).toHaveLength(2);
    expect(result.preLayoutTransient).toBe(false);
  });

  it("flags NaN x coordinate", () => {
    const result = validateRect(
      "register_scope",
      rect(Number.NaN, 0, 40, 40),
      100,
      105,
    );
    expect(result.errors.length).toBeGreaterThanOrEqual(1);
    expect(result.errors.some((e) => e.includes("rect.x"))).toBe(true);
    expect(result.errors.some((e) => e.includes("not finite"))).toBe(true);
  });

  it("flags Infinity y coordinate", () => {
    const result = validateRect(
      "register_scope",
      rect(0, Number.POSITIVE_INFINITY, 40, 40),
      100,
      105,
    );
    expect(result.errors.length).toBeGreaterThanOrEqual(1);
    expect(result.errors.some((e) => e.includes("rect.y"))).toBe(true);
    expect(result.errors.some((e) => e.includes("not finite"))).toBe(true);
  });

  it("collects errors for multiple bad components on one rect", () => {
    const result = validateRect(
      "register_scope",
      rect(Number.NaN, Number.NaN, -1, -1),
      100,
      105,
    );
    // Two non-finite errors (x, y) plus two non-positive errors (width,
    // height). The non-positive checks are skipped for non-finite values,
    // but width = -1 and height = -1 ARE finite, so they fire. The
    // plausible-scale check skips non-finite values too (`pushPlausibleScaleErrors`
    // continues on `!Number.isFinite(value)`), so x and y do not
    // contribute a second pair of errors there — that's why the total
    // is exactly 4 rather than 6. Note also that `-1` is a true negative,
    // not a zero, so the pre-layout-transient detection (which requires
    // a zero, not a negative) does not fire and the dim errors stay in
    // the error channel.
    expect(result.errors.length).toBe(4);
  });

  it("flags a coordinate far outside the plausible viewport range", () => {
    const result = validateRect(
      "register_scope",
      rect(50_000_000, 0, 40, 40),
      100,
      105,
    );
    expect(result.errors.length).toBeGreaterThanOrEqual(1);
    expect(
      result.errors.some((e) => e.includes("plausible viewport range")),
    ).toBe(true);
  });

  it("does not flag a coordinate inside the plausible viewport range", () => {
    // -100 is a legal off-screen virtualizer position.
    const result = validateRect(
      "register_scope",
      rect(-100, -200, 40, 40),
      100,
      105,
    );
    expect(result.errors).toEqual([]);
  });

  it("warns on stale rect sample (>16ms old)", () => {
    // sampled at 100ms, validating at 130ms → 30ms old → warning.
    const result = validateRect("update_rect", plausibleRect(), 100, 130);
    expect(result.warnings).toHaveLength(1);
    expect(result.warnings[0]).toMatch(/rect sample is 30\.0ms old/);
  });

  it("does not warn on a fresh rect sample (<= 16ms old)", () => {
    const result = validateRect("update_rect", plausibleRect(), 100, 116);
    expect(result.warnings).toEqual([]);
  });

  it("does not warn when the sample timestamp is the same tick as now", () => {
    const result = validateRect("update_rect", plausibleRect(), 100, 100);
    expect(result.warnings).toEqual([]);
  });

  it("ignores non-finite timestamps for the staleness check", () => {
    const result = validateRect(
      "update_rect",
      plausibleRect(),
      Number.NaN,
      100,
    );
    // No warning even though NaN < anything compares falsy.
    expect(result.warnings).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// validateAndLogRect — dev-mode gating + console output
// ---------------------------------------------------------------------------

describe("validateAndLogRect", () => {
  let errorSpy: ReturnType<typeof vi.spyOn>;
  let warnSpy: ReturnType<typeof vi.spyOn>;

  // Tests pass `enabled` directly to `validateAndLogRect` rather than
  // stubbing `import.meta.env.DEV` — Vitest's browser-mode env stubbing
  // does not reliably propagate `DEV` through the runtime, and the
  // function's optional `enabled` parameter exists precisely for this
  // dependency-injected test path.

  beforeEach(() => {
    errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    // Reset the per-(op, fq) pre-layout-transient dedup set so a test
    // that asserts on first-occurrence behaviour does not see state
    // from a prior test case.
    __resetPreLayoutTransientLog();
  });

  afterEach(() => {
    errorSpy.mockRestore();
    warnSpy.mockRestore();
    vi.unstubAllEnvs();
  });

  it("is a no-op when enabled = false", () => {
    const result = validateAndLogRect(
      "register_scope",
      asFq("/window/x"),
      rect(Number.NaN, 0, 40, 40),
      0,
      /* enabled */ false,
    );
    expect(result.errors).toEqual([]);
    expect(result.warnings).toEqual([]);
    expect(errorSpy).not.toHaveBeenCalled();
    expect(warnSpy).not.toHaveBeenCalled();
  });

  it("logs each error via console.error when enabled = true", () => {
    const result = validateAndLogRect(
      "register_scope",
      asFq("/window/x"),
      rect(Number.NaN, 0, -1, 40),
      performance.now(),
      /* enabled */ true,
    );
    expect(result.errors.length).toBeGreaterThanOrEqual(2);
    expect(errorSpy.mock.calls.length).toBe(result.errors.length);
    // The structured tag carries the op and FQM.
    expect(errorSpy.mock.calls[0]?.[0]).toContain("register_scope");
    expect(errorSpy.mock.calls[0]?.[0]).toContain("/window/x");
  });

  it("logs warnings via console.warn when enabled = true", () => {
    // Force staleness by passing an old sampledAt timestamp.
    const sampledAt = performance.now() - 100;
    const result = validateAndLogRect(
      "update_rect",
      asFq("/window/y"),
      plausibleRect(),
      sampledAt,
      /* enabled */ true,
    );
    expect(result.warnings.length).toBeGreaterThanOrEqual(1);
    expect(warnSpy.mock.calls.length).toBe(result.warnings.length);
    expect(warnSpy.mock.calls[0]?.[0]).toContain("update_rect");
  });

  it("logs nothing for a clean rect when enabled = true", () => {
    const result = validateAndLogRect(
      "register_scope",
      asFq("/window/z"),
      plausibleRect(),
      performance.now(),
      /* enabled */ true,
    );
    expect(result.errors).toEqual([]);
    expect(result.warnings).toEqual([]);
    expect(errorSpy).not.toHaveBeenCalled();
    expect(warnSpy).not.toHaveBeenCalled();
  });

  it("never throws on bad input", () => {
    expect(() =>
      validateAndLogRect(
        "register_scope",
        asFq("/window/x"),
        rect(Number.NaN, Number.POSITIVE_INFINITY, -1, -1),
        Number.NaN,
        /* enabled */ true,
      ),
    ).not.toThrow();
  });

  it("logs the pre-layout-transient warning once per (op, fq), not on every re-register", () => {
    // A zero-dim rect on `register_scope` is the pre-layout shape. The
    // first occurrence per (op, fq) is informative; later occurrences
    // are noise — StrictMode double-mount, ResizeObserver fire-on-mount,
    // and virtualizer placeholder→real-mount swaps repeatedly call
    // through the validator with the same FQM during the registration →
    // first-layout transition.
    const fq = asFq("/window/transient");
    for (let i = 0; i < 3; i++) {
      validateAndLogRect(
        "register_scope",
        fq,
        rect(0, 0, 100, 0),
        performance.now(),
        /* enabled */ true,
      );
    }
    expect(warnSpy.mock.calls.length).toBe(1);
    expect(warnSpy.mock.calls[0]?.[1]).toContain("zero dimension");
    // No errors emitted — this is the pre-layout transient path.
    expect(errorSpy).not.toHaveBeenCalled();
  });

  it("logs the pre-layout-transient warning per distinct (op, fq) pair", () => {
    // The dedup key is `(op, fq)` so a different op tag or a different
    // FQM is a fresh first occurrence. The validator's `RectValidationOp`
    // union still recognises `register_zone` (a legacy alias preserved
    // for callers that have not yet migrated); using it alongside
    // `register_scope` exercises the (op, fq) compound key.
    validateAndLogRect(
      "register_scope",
      asFq("/window/a"),
      rect(0, 0, 0, 0),
      performance.now(),
      /* enabled */ true,
    );
    validateAndLogRect(
      "register_zone",
      asFq("/window/a"),
      rect(0, 0, 0, 0),
      performance.now(),
      /* enabled */ true,
    );
    validateAndLogRect(
      "register_scope",
      asFq("/window/b"),
      rect(0, 0, 0, 0),
      performance.now(),
      /* enabled */ true,
    );
    expect(warnSpy.mock.calls.length).toBe(3);
  });

  it("zero-dim rects on update_rect log to console.error once per (op, fq) (deduped)", () => {
    // After layout, a zero dim is a real bug — ResizeObserver only
    // fires post-layout. The first occurrence per (op, fq) emits;
    // subsequent ones are deduped to keep test environments (where
    // ResizeObserver fires every frame on an unlaid-out node) from
    // drowning the channel with the same payload.
    for (let i = 0; i < 3; i++) {
      validateAndLogRect(
        "update_rect",
        asFq("/window/post-layout"),
        rect(0, 0, 100, 0),
        performance.now(),
        /* enabled */ true,
      );
    }
    expect(errorSpy.mock.calls.length).toBe(1);
    expect(warnSpy).not.toHaveBeenCalled();
  });

  it("zero-dim rects on update_rect re-log per distinct (op, fq) pair", () => {
    // The dedup key is `(op, fq)` so a different FQM is a fresh first
    // occurrence even on the same op tag.
    validateAndLogRect(
      "update_rect",
      asFq("/window/a"),
      rect(0, 0, 100, 0),
      performance.now(),
      /* enabled */ true,
    );
    validateAndLogRect(
      "update_rect",
      asFq("/window/b"),
      rect(0, 0, 100, 0),
      performance.now(),
      /* enabled */ true,
    );
    expect(errorSpy.mock.calls.length).toBe(2);
  });

  it("negative dimensions still log to console.error on register_scope (not transient)", () => {
    // A negative dim is not a pre-layout shape (`getBoundingClientRect()`
    // never returns negatives), so it stays in the error channel even
    // on the registration path.
    validateAndLogRect(
      "register_scope",
      asFq("/window/neg"),
      rect(0, 0, -10, 40),
      performance.now(),
      /* enabled */ true,
    );
    expect(errorSpy.mock.calls.length).toBe(1);
    expect(warnSpy).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// isDevModeRectValidationEnabled
// ---------------------------------------------------------------------------

describe("isDevModeRectValidationEnabled", () => {
  it("returns a boolean without throwing", () => {
    // The actual return value depends on the Vite build mode, which
    // Vitest's browser harness does not let us stub reliably. The
    // contract this test pins is "the helper does not throw and
    // produces a strict boolean" — production callers route through
    // it by default; unit tests of `validateAndLogRect` pass
    // `enabled` explicitly to avoid environment dependence.
    const result = isDevModeRectValidationEnabled();
    expect(typeof result).toBe("boolean");
  });
});
