/**
 * Dev-mode validators for rects shipped to the spatial-nav kernel.
 *
 * # Why
 *
 * The kernel's geometric pick (cardinal nav, `nav.{up,down,left,right}`)
 * is correct *iff* every candidate rect in the same layer was sampled in
 * the same coordinate system. The contract is **viewport-relative,
 * sampled by `getBoundingClientRect()`, refreshed on ancestor scroll**.
 * If any callsite ships a NaN coordinate, a negative dimension, a
 * cached document-relative rect, or a stale rect that predates a scroll,
 * beam search runs on broken geometry and the kernel produces wrong
 * answers silently — no exceptions, no warnings, just bad nav.
 *
 * The validators here are observability-only: in dev mode they
 * `console.error` / `console.warn` on detected violations; in production
 * they are no-ops (the entire branch is gated on `import.meta.env.DEV`,
 * the Vite-injected dev-mode flag). They never throw — the
 * registration / update IPC must still go through so the rest of the
 * registry stays consistent.
 *
 * # What is checked
 *
 * - **Finite coordinates** — `x`, `y`, `width`, `height` are not `NaN`,
 *   `+Infinity`, or `-Infinity`.
 * - **Positive dimensions** — `width > 0` and `height > 0`. The op
 *   distinguishes two cases:
 *
 *   - On `register_scope` / `register_zone` (initial registration), a
 *     zero in either dimension is treated as a *pre-layout transient*:
 *     `getBoundingClientRect()` legitimately returns rects with zero
 *     dims for `display: none`, just-mounted-but-not-yet-laid-out, and
 *     detached nodes (in test environments, jsdom-style flex/grid
 *     containers commonly produce `width × 0` zones until the first
 *     layout pass). That's not a coordinate-system bug — it's "the
 *     registration `useEffect` ran before the first layout pass." Such
 *     rects emit a single one-shot `console.warn` per `(op, fq)` rather
 *     than per-component `console.error`, so the channel stays clean
 *     during the registration → first-layout transition.
 *
 *   - On `update_rect`, a zero in either dimension is a real error.
 *     Update-rect runs from `ResizeObserver` and the ancestor-scroll
 *     listener, both of which fire only after layout has occurred. A
 *     zero dim at this point means the kernel will register a
 *     persistent broken rect, not a transient one — the violation must
 *     surface.
 * - **Plausible scale** — coordinates inside `[-1e6, 1e6]`. A rect at
 *   `(50000, 50000)` is almost always document-relative (the user
 *   computed `node.offsetTop` instead of `getBoundingClientRect().top`)
 *   and would silently mis-rank against viewport-relative siblings.
 * - **Fresh sample** — the timestamp passed alongside the rect is no
 *   more than one animation frame (16 ms) older than `performance.now()`
 *   at validation time. Stale rects predate a scroll the kernel hasn't
 *   seen yet, so beam search runs on geometry the user no longer sees.
 *   The contract is that callers capture `performance.now()` at the
 *   exact callsite that sampled the rect via `getBoundingClientRect()`
 *   and thread it through the IPC adapter; capturing the timestamp at
 *   the adapter (one tick later) is a contract violation that defeats
 *   the staleness check.
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
 * Op tag identifying which IPC adapter sourced the rect. Mirrors the
 * `op` field on the kernel-side `tracing` events so log readers can
 * correlate the two sides.
 */
export type RectValidationOp =
  | "register_scope"
  | "register_zone"
  | "update_rect";

/**
 * Result of validating one rect. Each `string` in `errors` describes
 * one violation in human-readable form, suitable for `console.error`.
 *
 * `errors` carries hard violations (NaN, negative dim, post-layout
 * zero dim on `update_rect`, etc.); `warnings` carries soft violations
 * (stale timestamp, pre-layout transient zero dim on
 * `register_scope` / `register_zone`). The split lets the caller pick
 * `console.error` vs `console.warn` per category.
 *
 * `preLayoutTransient` is `true` when the rect has at least one zero
 * dimension AND the op is a registration (`register_scope` /
 * `register_zone`) — the structural shape `getBoundingClientRect()`
 * legitimately produces for `display: none`,
 * just-mounted-but-not-yet-laid-out, and detached nodes during initial
 * registration. Callers (`validateAndLogRect`) use this to dedup the
 * warning per `(op, fq)` so a re-registering scope (StrictMode
 * double-mount, virtualizer placeholder→real-mount swap) does not
 * spam the channel.
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
  op: RectValidationOp,
  rect: Rect,
  sampledAtMs: number,
  nowMs: number = performance.now(),
): RectValidationResult {
  const errors: string[] = [];
  const warnings: string[] = [];

  pushFiniteErrors(rect, errors);
  // Zero-dim handling depends on the op:
  // - On register_scope / register_zone, a zero dim is a pre-layout
  //   transient — layout has not yet completed, and the next
  //   ResizeObserver fire will populate the real rect. Surface as a
  //   one-shot warning per (op, fq) so the channel stays clean during
  //   the registration → first-layout transition.
  // - On update_rect, a zero dim is a real error. Update fires from
  //   ResizeObserver / ancestor-scroll listener, both of which run
  //   only after layout, so a zero dim here means the kernel will
  //   record a persistent broken rect.
  const isRegistration = op === "register_scope" || op === "register_zone";
  const hasZeroDim = isZeroDim(rect);
  const preLayoutTransient = isRegistration && hasZeroDim;
  if (preLayoutTransient) {
    // The rect has a zero dimension, but that's expected on initial layout before resive
  } else {
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
 * above and a follow-up "non-positive" message would be noise.
 */
function pushPositiveDimensionErrors(rect: Rect, errors: string[]): void {
  if (Number.isFinite(rect.width) && rect.width <= 0) {
    errors.push(
      `rect.width must be > 0 (got ${rect.width}); a zero-size rect breaks beam search distance math`,
    );
  }
  if (Number.isFinite(rect.height) && rect.height <= 0) {
    errors.push(
      `rect.height must be > 0 (got ${rect.height}); a zero-size rect breaks beam search distance math`,
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
 * caller's IPC `invoke` proceeds either way — this validator is
 * observability, not a circuit breaker.
 *
 * Each error is logged with `console.error`; each warning with
 * `console.warn`. The first argument is a structured tag identifying
 * the op and FQM so log filters / source-map consumers can locate the
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
  op: RectValidationOp,
  fq: FullyQualifiedMoniker,
  rect: Rect,
  sampledAtMs: number,
  enabled: boolean = isDevModeRectValidationEnabled(),
): RectValidationResult {
  if (!enabled) {
    return { errors: [], warnings: [], preLayoutTransient: false };
  }
  const result = validateRect(op, rect, sampledAtMs);

  // Zero-dim emissions get one-shot dedup per (op, fq) regardless of
  // whether they surfaced as a warning (registration path) or an error
  // (update_rect path). The first occurrence per (op, fq) is informative
  // — "this scope was registered with no layout" or "the kernel saw a
  // post-layout zero-dim rect" — but a re-rendering component, a rapid
  // ResizeObserver burst, or a test environment that fires update_rect
  // every frame on an unlaid-out node will repeat the same payload. The
  // dedup keeps the channel clean for genuinely new offenders.
  //
  // Other errors (NaN, negative, plausible-scale) are NOT deduped: each
  // one is structurally distinct enough that repetition is itself a
  // signal that something is off.
  const zeroDim = isZeroDim(rect);
  const zeroDimKey = zeroDim ? `${op}|${fq}` : null;
  const zeroDimAlreadyLogged =
    zeroDimKey !== null && loggedZeroDim.has(zeroDimKey);

  for (const error of result.errors) {
    if (
      zeroDimAlreadyLogged &&
      (error.startsWith("rect.width must be > 0") ||
        error.startsWith("rect.height must be > 0"))
    ) {
      continue;
    }
    console.error(`[spatial-nav][${op}][${fq}] rect validation error:`, error, {
      rect,
    });
  }
  for (const warning of result.warnings) {
    if (
      zeroDimAlreadyLogged &&
      warning.startsWith("rect has a zero dimension")
    ) {
      continue;
    }
    console.warn(
      `[spatial-nav][${op}][${fq}] rect validation warning:`,
      warning,
      { rect },
    );
  }

  if (zeroDimKey !== null) {
    loggedZeroDim.add(zeroDimKey);
  }
  return result;
}

/**
 * Per-`(op, fq)` dedup set for zero-dim rect emissions (both the
 * registration-path warning and the update_rect-path error). A
 * registered scope that mounts before its first layout pass produces a
 * zero-dim rect; the first emission is informative but quickly turns
 * to noise if the scope re-registers across StrictMode double-mount,
 * virtualizer placeholder→real-mount swaps, or rapid re-renders. On
 * the update_rect side, test environments routinely fire ResizeObserver
 * with a zero-dim rect every frame because jsdom doesn't compute layout;
 * deduping per (op, fq) collapses that burst to one event.
 *
 * The set is process-scoped so the same FQM, even after
 * unregister/re-register, only logs once per session — that matches
 * the "best-effort observability, never noise" tone of the rest of the
 * validators. The trade-off is that a real bug that re-introduces a
 * zero dim after a previously-logged occurrence will be silent until
 * the dedup is reset; in practice the first occurrence is enough to
 * surface the bug class to a log reader.
 *
 * Test-only: `__resetPreLayoutTransientLog` clears the set so individual
 * tests can pin first-occurrence behaviour without leaking state across
 * test cases.
 */
const loggedZeroDim = new Set<string>();

/**
 * Reset the zero-dim dedup set. Test-only — production code never
 * calls this; the set is intentionally process-scoped at runtime. The
 * name is kept as `__resetPreLayoutTransientLog` for backward
 * compatibility with existing test imports; functionally it now resets
 * the broader zero-dim dedup set, which is a strict superset of the
 * old pre-layout-transient set.
 */
export function __resetPreLayoutTransientLog(): void {
  loggedZeroDim.clear();
}
