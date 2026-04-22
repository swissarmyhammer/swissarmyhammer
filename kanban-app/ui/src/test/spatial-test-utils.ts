/**
 * Shared helpers for spatial-nav browser tests.
 */

/**
 * How long to wait for the macrotask queue to drain and let
 * `requestAnimationFrame` side effects (e.g. `spatial_focus_first_in_layer`
 * after a `FocusLayer` push) run to completion before re-asserting.
 *
 * 50ms is empirically enough on the CI vitest-browser harness: one RAF
 * (~16ms) plus a `focus-changed` event round trip through the store.
 * Centralizing avoids magic numbers in the test suite and makes it a
 * single place to tune if the harness grows slower.
 */
export const EVENT_LOOP_SETTLE_MS = 50;

/** Yield to the macrotask queue for `EVENT_LOOP_SETTLE_MS` milliseconds. */
export function settleEventLoop(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, EVENT_LOOP_SETTLE_MS));
}
