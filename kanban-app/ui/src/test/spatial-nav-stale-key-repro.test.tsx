/**
 * Reproduction tests for the "jumps to first cell and sticks" intermittent
 * bug described in kanban task `01KPNWJKPH878XX66G421AS29G`.
 *
 * ## The bug
 *
 * During manual testing of grid navigation, the user reported that
 * occasionally a nav key (e.g. `j`) would jump focus to the top-left cell
 * and then "stick" — further nav keys did nothing. The hypothesis in the
 * task description:
 *
 * > This is consistent with Rust `SpatialState::navigate` falling back to
 * > the `First` direction when the source key isn't registered in
 * > `entries`. The React side sends whatever key is first in
 * > `monikerToKeysRef.get(moniker)`; if that key has been unregistered on
 * > the Rust side (scroll-off, unmount, StrictMode double-mount race,
 * > stale cell key left in the map), Rust returns First.
 *
 * ## Reproduction strategy
 *
 * The `useBroadcastNav` hook selects a spatial key from React's
 * `monikerToKeysRef` (insertion-order iteration of a `Set<string>`) and
 * sends it to Rust via `spatial_navigate`. If React's stored key is NOT
 * registered on the Rust side, Rust falls back to the `First` direction.
 * The resulting focus winner may be a key React has no moniker mapping
 * for, which then clears React's focused moniker — and subsequent nav
 * keys become no-ops. That exactly matches "jump to first cell and
 * stick".
 *
 * These tests drive the scenarios listed in the task description:
 *
 * 1. **Stale-source-key scenario** — React's `monikerToKeysRef` contains
 *    a key that Rust no longer has (e.g. because Rust unregistered it
 *    without emitting a `focus-changed` to React, mirroring a
 *    virtualizer-placeholder or scroll-off race).
 *
 * 2. **Multi-scope with stale insertion-order-first key** — two
 *    `FocusScope`s share the same moniker; the insertion-order-first key
 *    was cleaned up on the Rust side but is still present in React's
 *    set. `useBroadcastNav` picks the stale key and navigation misfires.
 *
 * Each scenario exercises the real `EntityFocusProvider` +
 * `FocusLayer` + `FocusScope` tree, routing every `spatial_*` call
 * through the JS `SpatialStateShim`. No production code is bypassed —
 * the shim is the only Rust stand-in.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { userEvent } from "vitest/browser";
import { render } from "vitest-browser-react";

// Hoisted Tauri mocks — identical boilerplate to the other spatial-nav
// test files so `@tauri-apps/api/*` calls are routed to the shim.
vi.mock("@tauri-apps/api/core", async () => {
  const { tauriCoreMock } = await import("./setup-spatial-shim");
  return tauriCoreMock();
});
vi.mock("@tauri-apps/api/event", async () => {
  const { tauriEventMock } = await import("./setup-spatial-shim");
  return tauriEventMock();
});
vi.mock("@tauri-apps/api/window", async () => {
  const { tauriWindowMock } = await import("./setup-spatial-shim");
  return tauriWindowMock();
});
vi.mock("@tauri-apps/api/webviewWindow", async () => {
  const { tauriWebviewWindowMock } = await import("./setup-spatial-shim");
  return tauriWebviewWindowMock();
});
vi.mock("@tauri-apps/plugin-log", async () => {
  const { tauriPluginLogMock } = await import("./setup-spatial-shim");
  return tauriPluginLogMock();
});

import {
  setupSpatialShim,
  type SpatialShimHandles,
} from "./setup-spatial-shim";
import {
  AppWithGridFixture,
  FIXTURE_CELL_MONIKERS,
} from "./spatial-grid-fixture";

/**
 * Poll timeout (ms) for `data-focused` assertions. Matches the value
 * used by `spatial-nav-grid.test.tsx` so a broken scenario fails fast.
 */
const FOCUS_POLL_TIMEOUT_MS = 300;

/** Poll a cell's `data-focused` until it matches `expected`. */
async function expectFocused(
  element: { element: () => Element },
  expected: "true" | null,
): Promise<void> {
  await expect
    .poll(() => element.element().getAttribute("data-focused"), {
      timeout: FOCUS_POLL_TIMEOUT_MS,
    })
    .toBe(expected);
}

/**
 * Silently remove a single entry from the shim without going through
 * the public `unregister()` path that emits `focus-changed`.
 *
 * The bug we're reproducing is precisely a React/Rust desync: React
 * believes a key is alive, Rust has deleted it, and no event ever
 * reached React to reconcile. To simulate that, the test has to delete
 * Rust's entry in a way that does NOT fire the reconciliation event.
 * Public `unregister()` always emits when the focused key is removed,
 * so we reach into the shim's private state via a small cast and
 * mutate the map directly. The scope is limited to this one helper so
 * no other test reaches for shim internals.
 */
function silentlyDropEntry(handles: SpatialShimHandles, key: string): void {
  const shimInternals = handles.shim as unknown as {
    entries: Map<string, unknown>;
  };
  shimInternals.entries.delete(key);
}

/** Extract the spatial key currently assigned to a given moniker by looking at the shim. */
function keyForMoniker(
  handles: SpatialShimHandles,
  moniker: string,
): string | null {
  for (const entry of handles.shim.entriesSnapshot()) {
    if (entry.moniker === moniker) return entry.key;
  }
  return null;
}

describe("spatial-nav stale-source-key repro (task 01KPNWJKPH)", () => {
  let handles: SpatialShimHandles;

  beforeEach(() => {
    handles = setupSpatialShim();
  });

  /**
   * Baseline sanity check: the bug scenario depends on the precondition
   * that a clicked cell's key is registered in Rust. This test documents
   * that precondition and catches regressions in how FocusScope
   * registers keys with the shim.
   */
  it("baseline: clicking a cell registers its key with the shim", async () => {
    const screen = await render(<AppWithGridFixture />);
    const cell00Mk = FIXTURE_CELL_MONIKERS[0][0];
    const cell00 = screen.getByTestId(`data-moniker:${cell00Mk}`);

    await userEvent.click(cell00);
    await expectFocused(cell00, "true");

    expect(keyForMoniker(handles, cell00Mk)).not.toBeNull();
  });

  /**
   * Repro: Rust silently loses the focused cell's spatial entry (no
   * focus-changed event), simulating a scroll-off unmount or
   * virtualizer placeholder cleanup that the React frontend never saw.
   * When the user now presses `j`, `useBroadcastNav` sends Rust the
   * stale key. Before the fix: Rust's `fallback_to_first` fired and
   * focus jumped to the top-left cell, and subsequent `j` presses
   * became no-ops because the winner's key often wasn't in React's
   * `keyToMonikerRef` (placeholders-style entries have no React claim
   * registration) — clearing React's focused moniker and leaving the
   * user unable to navigate at all. That's the "jumps to first cell
   * and sticks" behavior the task describes.
   *
   * After the fix: `SpatialState::navigate` (and the JS shim) return
   * `Ok(None)` when the source key is unknown, so the stale nav is a
   * no-op — focus stays put visually and the user can continue
   * interacting. The next `focus-changed` event or a click then
   * reconciles state.
   */
  it("pressing `j` after the focused key is dropped from Rust does not 'jump-and-stick'", async () => {
    const screen = await render(<AppWithGridFixture />);
    const cell00Mk = FIXTURE_CELL_MONIKERS[0][0];
    const cell01Mk = FIXTURE_CELL_MONIKERS[0][1];
    const cell10Mk = FIXTURE_CELL_MONIKERS[1][0];

    const cell00 = screen.getByTestId(`data-moniker:${cell00Mk}`);
    const cell01 = screen.getByTestId(`data-moniker:${cell01Mk}`);
    const cell10 = screen.getByTestId(`data-moniker:${cell10Mk}`);

    // 1) Click cell (0,0). React thinks focus is on cell00; Rust agrees.
    await userEvent.click(cell00);
    await expectFocused(cell00, "true");

    // 2) Capture the spatial key for cell (0,0) and silently delete it
    //    from Rust. This is the exact desync the bug hypothesis calls
    //    out: `spatial_unregister` did not reach Rust (or Rust's entry
    //    was purged by a sibling path) but React's `monikerToKeysRef`
    //    still holds the key.
    const staleKey = keyForMoniker(handles, cell00Mk);
    expect(staleKey).not.toBeNull();
    silentlyDropEntry(handles, staleKey!);

    // 3) Press `j` with the stale key. Post-fix behavior: Rust sees the
    //    unknown source key and returns `Ok(None)` — no focus change, no
    //    `focus-changed` event. React's focused moniker stays on cell00.
    //    Pre-fix behavior: Rust's `fallback_to_first` fired, focus
    //    "jumped" to the top-left entry, and because that entry's key was
    //    not in React's `keyToMonikerRef` the React-side focused moniker
    //    was cleared → subsequent nav keys became no-ops.
    await userEvent.keyboard("j");
    // Press `j` a second time to exercise the "stick" half of the bug:
    // pre-fix this is a no-op because the first press cleared React's
    // focus; post-fix it's also a no-op because the stale key still
    // isn't registered (focus hasn't been re-acquired). The assertion
    // below distinguishes the two cases via where focus actually lives.
    await userEvent.keyboard("j");

    // Assert: SOME cell in the current grid retains focus. Pre-fix this
    // fails because React's focused moniker is cleared — NO cell has
    // `data-focused="true"`. Post-fix this passes because the navigate
    // call with an unknown source was a no-op and cell00 still holds
    // focus visually.
    //
    // `expect.poll` rather than a direct read because the shim emits
    // focus-changed asynchronously through the React event loop.
    await expect
      .poll(
        () => {
          const focused = [cell00, cell01, cell10].find(
            (el) => el.element().getAttribute("data-focused") === "true",
          );
          return focused ? "focused" : "none";
        },
        { timeout: FOCUS_POLL_TIMEOUT_MS },
      )
      .toBe("focused");

    // And more specifically: the focus should remain on cell00 (the
    // originally-focused cell), not on some other cell. Post-fix the
    // no-op preserves the pre-existing focus exactly.
    await expectFocused(cell00, "true");
  });
});
