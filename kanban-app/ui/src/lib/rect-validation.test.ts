/**
 * Unit tests for `rect-validation.ts`.
 *
 * Pins the invariants the dev-mode validator enforces:
 *
 *   1. A valid rect produces no errors and no warnings.
 *   2. A non-finite component produces exactly one error per offender.
 *   3. A negative width or height produces an error.
 *   4. A zero-dim rect surfaces through `preLayoutTransient` and emits
 *      no console message.
 *   5. Coordinates outside the plausible viewport range produce an error.
 *   6. A stale sample timestamp produces a warning.
 *
 * The validator is **observability-only** — it never throws and never
 * blocks the caller. Tests assert on returned `errors` / `warnings`
 * arrays and on `console.error` / `console.warn` spies attached via
 * Vitest.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  validateRect,
  validateAndLogRect,
  isDevModeRectValidationEnabled,
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
  it("passes a plausible viewport-relative rect", () => {
    const result = validateRect(plausibleRect(), 100, 105);
    expect(result.errors).toEqual([]);
    expect(result.warnings).toEqual([]);
    expect(result.preLayoutTransient).toBe(false);
  });

  it("flags negative width as an error", () => {
    const result = validateRect(rect(0, 0, -10, 40), 100, 105);
    expect(result.errors).toHaveLength(1);
    expect(result.errors[0]).toMatch(/width must be >= 0/);
    expect(result.preLayoutTransient).toBe(false);
  });

  it("flags negative height as an error", () => {
    const result = validateRect(rect(0, 0, 40, -1), 100, 105);
    expect(result.errors).toHaveLength(1);
    expect(result.errors[0]).toMatch(/height must be >= 0/);
    expect(result.preLayoutTransient).toBe(false);
  });

  it("treats a zero-dim rect as a pre-layout transient (no error, no warning)", () => {
    const result = validateRect(rect(0, 0, 100, 0), 100, 105);
    expect(result.errors).toEqual([]);
    expect(result.warnings).toEqual([]);
    expect(result.preLayoutTransient).toBe(true);
  });

  it("treats a both-zero rect as a pre-layout transient (no error, no warning)", () => {
    const result = validateRect(rect(0, 0, 0, 0), 100, 105);
    expect(result.errors).toEqual([]);
    expect(result.warnings).toEqual([]);
    expect(result.preLayoutTransient).toBe(true);
  });

  it("flags NaN x coordinate", () => {
    const result = validateRect(rect(Number.NaN, 0, 40, 40), 100, 105);
    expect(result.errors.length).toBeGreaterThanOrEqual(1);
    expect(result.errors.some((e) => e.includes("rect.x"))).toBe(true);
    expect(result.errors.some((e) => e.includes("not finite"))).toBe(true);
  });

  it("flags Infinity y coordinate", () => {
    const result = validateRect(
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
      rect(Number.NaN, Number.NaN, -1, -1),
      100,
      105,
    );
    // Two non-finite errors (x, y) plus two negative-dim errors
    // (width, height). The plausible-scale check skips non-finite
    // values, so x and y do not contribute a second pair of errors.
    expect(result.errors.length).toBe(4);
  });

  it("flags a coordinate far outside the plausible viewport range", () => {
    const result = validateRect(rect(50_000_000, 0, 40, 40), 100, 105);
    expect(result.errors.length).toBeGreaterThanOrEqual(1);
    expect(
      result.errors.some((e) => e.includes("plausible viewport range")),
    ).toBe(true);
  });

  it("does not flag a coordinate inside the plausible viewport range", () => {
    // -100 is a legal off-screen virtualizer position.
    const result = validateRect(rect(-100, -200, 40, 40), 100, 105);
    expect(result.errors).toEqual([]);
  });

  it("warns on stale rect sample (>16ms old)", () => {
    // sampled at 100ms, validating at 130ms → 30ms old → warning.
    const result = validateRect(plausibleRect(), 100, 130);
    expect(result.warnings).toHaveLength(1);
    expect(result.warnings[0]).toMatch(/rect sample is 30\.0ms old/);
  });

  it("does not warn on a fresh rect sample (<= 16ms old)", () => {
    const result = validateRect(plausibleRect(), 100, 116);
    expect(result.warnings).toEqual([]);
  });

  it("does not warn when the sample timestamp is the same tick as now", () => {
    const result = validateRect(plausibleRect(), 100, 100);
    expect(result.warnings).toEqual([]);
  });

  it("ignores non-finite timestamps for the staleness check", () => {
    const result = validateRect(plausibleRect(), Number.NaN, 100);
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
  });

  afterEach(() => {
    errorSpy.mockRestore();
    warnSpy.mockRestore();
    vi.unstubAllEnvs();
  });

  it("is a no-op when enabled = false", () => {
    const result = validateAndLogRect(
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
      asFq("/window/x"),
      rect(Number.NaN, 0, -1, 40),
      performance.now(),
      /* enabled */ true,
    );
    expect(result.errors.length).toBeGreaterThanOrEqual(2);
    expect(errorSpy.mock.calls.length).toBe(result.errors.length);
    // The structured tag carries the FQM.
    expect(errorSpy.mock.calls[0]?.[0]).toContain("/window/x");
  });

  it("logs warnings via console.warn when enabled = true", () => {
    // Force staleness by passing an old sampledAt timestamp.
    const sampledAt = performance.now() - 100;
    const result = validateAndLogRect(
      asFq("/window/y"),
      plausibleRect(),
      sampledAt,
      /* enabled */ true,
    );
    expect(result.warnings.length).toBeGreaterThanOrEqual(1);
    expect(warnSpy.mock.calls.length).toBe(result.warnings.length);
    expect(warnSpy.mock.calls[0]?.[0]).toContain("/window/y");
  });

  it("logs nothing for a clean rect when enabled = true", () => {
    const result = validateAndLogRect(
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
        asFq("/window/x"),
        rect(Number.NaN, Number.POSITIVE_INFINITY, -1, -1),
        Number.NaN,
        /* enabled */ true,
      ),
    ).not.toThrow();
  });

  it("logs nothing for a zero-dim rect (pre-layout transient)", () => {
    const result = validateAndLogRect(
      asFq("/window/transient"),
      rect(0, 0, 100, 0),
      performance.now(),
      /* enabled */ true,
    );
    expect(result.preLayoutTransient).toBe(true);
    expect(errorSpy).not.toHaveBeenCalled();
    expect(warnSpy).not.toHaveBeenCalled();
  });

  it("negative dimensions log to console.error (not transient)", () => {
    // A negative dim is not a pre-layout shape (`getBoundingClientRect()`
    // never returns negatives).
    validateAndLogRect(
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
