/**
 * Dev-mode validators for rects flowing through the spatial-nav pipeline.
 *
 * # Why
 *
 * The kernel's geometric pick (cardinal nav, `nav.{up,down,left,right}`)
 * is correct *iff* every candidate rect in the same layer was sampled in
 * the same coordinate system. The contract is **viewport-relative,
 * sampled by `getBoundingClientRect()`**. If any callsite ships a NaN
 * coordinate, a negative dimension, a cached document-relative rect, or
 * a stale rect that predates a scroll, beam search runs on broken
 * geometry and the kernel produces wrong answers silently — no
 * exceptions, no warnings, just bad nav.
 *
 * The validators here are observability-only: in dev mode they
 * `console.error` / `console.warn` on detected violations; in production
 * they are no-ops (the entire branch is gated on `import.meta.env.DEV`,
 * the Vite-injected dev-mode flag). They never throw — the calling code
 * still proceeds so the rest of the registry stays consistent.
 *
 * # What is checked
 *
 * - **Finite coordinates** — `x`, `y`, `width`, `height` are not `NaN`,
 *   `+Infinity`, or `-Infinity`.
 * - **Plausible dimensions** — `width >= 0` and `height >= 0`. A
 *   zero-dim rect is the structural shape `getBoundingClientRect()`
 *   legitimately produces for `display: none`,
 *   just-mounted-but-not-yet-laid-out, and detached nodes; the
 *   validator surfaces it through the `preLayoutTransient` flag rather
 *   than a console message so the channel stays clean during the
 *   registration → first-layout transition. Negative dims are real
 *   errors.
 * - **Plausible scale** — coordinates inside `[-1e6, 1e6]`. A rect at
 *   `(50000, 50000)` is almost always document-relative (the user
 *   computed `node.offsetTop` instead of `getBoundingClientRect().top`)
 *   and would silently mis-rank against viewport-relative siblings.
 * - **Fresh sample** — the timestamp passed alongside the rect is no
 *   more than one animation frame (16 ms) older than `performance.now()`
 *   at validation time. Stale rects predate a scroll the kernel hasn't
 *   seen yet, so beam search runs on geometry the user no longer sees.
 *
 * The kernel side has its own `cfg(debug_assertions)` validators that
 * cover the same invariants — the TS side catches the bug before it
 * crosses the IPC boundary, the Rust side catches it if the TS
 * validator was bypassed (e.g. a different IPC adapter, a Rust-side
 * test fixture).
 *
 * # Cross-reference
 *
 * - `swissarmyhammer-focus/src/registry.rs` — kernel-side validators in
 *   `register_scope`, `register_zone`, `update_rect`.
 * - `swissarmyhammer-focus/README.md` `## Coordinate system` — the
 *   prose contract these validators enforce.
 */

import type { FullyQualifiedMoniker, Rect } from "@/types/spatial";

/**
 * Maximum age (in milliseconds) for a rect sample before the validator
 * flags it as stale. One animation frame at 60 fps is ~16.67 ms; we
 * round up to 16 ms because the rare 120 fps display still leaves
 * plenty of headroom for a same-tick sample-then-validate path.
 */
const STALE_RECT_MS = 16;

/**
 * Bound on plausible viewport-relative coordinates. Values outside
 * `[-LARGE_COORD_BOUND, LARGE_COORD_BOUND]` are flagged as likely
 * document-relative or otherwise corrupted. 1e6 px is two orders of
 * magnitude beyond any real desktop layout (8K display = 7680 px wide)
 * so the bound is loose enough to allow off-screen virtualizer
 * placeholders (negative rects, far-right rects) while still catching
 * the unit-error / wrong-coordinate-system cases the validator exists
 * to surface.
 */
const LARGE_COORD_BOUND = 1_000_000;

/**
 * Result of validating one rect. `errors` carries hard violations
 * (NaN, negative dim, implausible scale, etc.); `warnings` carries
 * soft violations (stale timestamp). `preLayoutTransient` is `true`
 * when the rect has at least one zero dimension — the structural
 * shape `getBoundingClientRect()` legitimately produces for
 * `display: none`, just-mounted-but-not-yet-laid-out, and detached
 * nodes; callers can branch on it to skip downstream work that
 * needs real geometry.
 */
export interface RectValidationResult {
  errors: string[];
  warnings: string[];
  preLayoutTransient: boolean;
}

/**
 * `true` when validators should run.
 *
 * Reads `import.meta.env.DEV` — the Vite-injected boolean that is `true`
 * in development builds and `false` in production builds. Mirrors the
 * dev-mode gating already used by `command-scope.tsx::warnOnceNoopSetter`,
 * so the validator's runtime behaviour matches the rest of the
 * dev-only diagnostics in this codebase.
 *
 * In Vitest's browser-mode tests the value comes from the same source;
 * tests that need to override it can use `vi.stubEnv("DEV", "true" |
 * "false")` (vitest's stub helper sets `import.meta.env.DEV`
 * accordingly).
 */
export function isDevModeRectValidationEnabled(): boolean {
  // Guard against `import.meta` being unavailable (extremely rare in
  // modern bundlers, but cheap to defend against — the failure mode
  // is "validator silently disabled", which is the safe default).
  if (typeof import.meta === "undefined") return false;
  return import.meta.env?.DEV === true;
}

/**
 * Inspect a rect for the four invariants documented at the top of this
 * module. Pure function — does not log, does not throw. Returns the
 * accumulated error / warning strings; an empty `errors` AND empty
 * `warnings` means the rect is good.
 *
 * `sampledAtMs` is the `performance.now()` timestamp captured at the
 * exact moment `getBoundingClientRect()` returned, used for the
 * staleness check. Pass `performance.now()` at the same callsite that
 * sampled the rect; the validator will compare against `nowMs` (also
 * `performance.now()` by default, overridable for deterministic tests).
 */
export function validateRect(
  rect: Rect,
  sampledAtMs: number,
  nowMs: number = performance.now(),
): RectValidationResult {
  const errors: string[] = [];
  const warnings: string[] = [];

  pushFiniteErrors(rect, errors);
  const preLayoutTransient = isZeroDim(rect);
  if (!preLayoutTransient) {
    pushPositiveDimensionErrors(rect, errors);
  }
  pushPlausibleScaleErrors(rect, errors);
  pushStalenessWarnings(sampledAtMs, nowMs, warnings);

  return { errors, warnings, preLayoutTransient };
}

/**
 * `true` when at least one of `rect.width` / `rect.height` is exactly
 * zero. Skips the check when either dim is non-finite — the finite
 * check already produced an error for that case, and a `NaN`/`Infinity`
 * dim is a different bug class than "not laid out yet".
 *
 * Negative dims are not pre-layout transient: a `-10` width is not what
 * `getBoundingClientRect()` produces for an unlaid-out node, so it
 * stays in the error path.
 */
function isZeroDim(rect: Rect): boolean {
  if (!Number.isFinite(rect.width) || !Number.isFinite(rect.height)) {
    return false;
  }
  if (rect.width < 0 || rect.height < 0) return false;
  return rect.width === 0 || rect.height === 0;
}

/**
 * Append one error per non-finite numeric component of `rect`.
 *
 * Each component is checked independently so a rect with two bad
 * components produces two errors — the log reader sees every offending
 * field at once, rather than fixing one and re-running to find the next.
 */
function pushFiniteErrors(rect: Rect, errors: string[]): void {
  const components: Array<["x" | "y" | "width" | "height", number]> = [
    ["x", rect.x],
    ["y", rect.y],
    ["width", rect.width],
    ["height", rect.height],
  ];
  for (const [name, value] of components) {
    if (!Number.isFinite(value)) {
      errors.push(
        `rect.${name} is not finite (got ${String(value)}); expected a real-valued pixel coordinate from getBoundingClientRect()`,
      );
    }
  }
}

/**
 * Append errors for non-positive width/height. Skips finite-failed
 * components since a non-finite value already produced its own error
 * above and a follow-up "non-positive" message would be noise. Skips
 * zero dims as well — those are surfaced through the
 * `preLayoutTransient` flag rather than as errors.
 */
function pushPositiveDimensionErrors(rect: Rect, errors: string[]): void {
  if (Number.isFinite(rect.width) && rect.width < 0) {
    errors.push(
      `rect.width must be >= 0 (got ${rect.width}); a negative-size rect breaks beam search distance math`,
    );
  }
  if (Number.isFinite(rect.height) && rect.height < 0) {
    errors.push(
      `rect.height must be >= 0 (got ${rect.height}); a negative-size rect breaks beam search distance math`,
    );
  }
}

/**
 * Append errors for coordinates that fall outside the plausible
 * viewport-relative bound. A rect at `(50000, 50000)` is almost
 * certainly document-relative (computed via `node.offsetTop` /
 * `node.offsetLeft` instead of `getBoundingClientRect()`), and beam
 * search would silently mis-rank it against viewport-relative siblings.
 */
function pushPlausibleScaleErrors(rect: Rect, errors: string[]): void {
  const components: Array<["x" | "y" | "width" | "height", number]> = [
    ["x", rect.x],
    ["y", rect.y],
    ["width", rect.width],
    ["height", rect.height],
  ];
  for (const [name, value] of components) {
    if (!Number.isFinite(value)) continue;
    if (Math.abs(value) > LARGE_COORD_BOUND) {
      errors.push(
        `rect.${name} = ${value} is outside the plausible viewport range (|x| ≤ ${LARGE_COORD_BOUND}); likely document-relative coordinates instead of viewport-relative`,
      );
    }
  }
}

/**
 * Append one staleness warning when the sample timestamp is older than
 * one animation frame. A stale rect predates a scroll the kernel
 * hasn't seen yet — beam search would run on geometry the user no
 * longer sees, picking the wrong neighbor.
 */
function pushStalenessWarnings(
  sampledAtMs: number,
  nowMs: number,
  warnings: string[],
): void {
  if (!Number.isFinite(sampledAtMs) || !Number.isFinite(nowMs)) return;
  const age = nowMs - sampledAtMs;
  if (age > STALE_RECT_MS) {
    warnings.push(
      `rect sample is ${age.toFixed(1)}ms old (max ${STALE_RECT_MS}ms); rect may predate an unobserved scroll and mislead beam search`,
    );
  }
}

/**
 * Validate a rect and log violations to the console. Returns the result
 * for callers that want to act on it (e.g. tests).
 *
 * No-op when dev-mode validation is disabled (production builds). The
 * caller's downstream work proceeds either way — this validator is
 * observability, not a circuit breaker.
 *
 * Each error is logged with `console.error`; each warning with
 * `console.warn`. The first argument is a structured tag identifying
 * the FQM so log filters / source-map consumers can locate the
 * offender; subsequent arguments include the rect and the violation
 * message.
 *
 * The optional `enabled` parameter overrides the dev-mode auto-detection
 * — production callers always pass `isDevModeRectValidationEnabled()`
 * (or omit and let the default fire), and tests pass `true` to assert
 * on the dev-mode behaviour without needing to stub Vite's
 * `import.meta.env.DEV` (which Vitest's browser-mode stubbing does not
 * always plumb through).
 */
export function validateAndLogRect(
  fq: FullyQualifiedMoniker,
  rect: Rect,
  sampledAtMs: number,
  enabled: boolean = isDevModeRectValidationEnabled(),
): RectValidationResult {
  if (!enabled) {
    return { errors: [], warnings: [], preLayoutTransient: false };
  }
  const result = validateRect(rect, sampledAtMs);

  for (const error of result.errors) {
    console.error(`[spatial-nav][${fq}] rect validation error:`, error, {
      rect,
    });
  }
  for (const warning of result.warnings) {
    console.warn(`[spatial-nav][${fq}] rect validation warning:`, warning, {
      rect,
    });
  }

  return result;
}
