/**
 * Tests covering the React/Rust stale-source-key desync scenario.
 *
 * ## History
 *
 * This file originally reproduced a "jumps to first cell and sticks" bug
 * where Rust's `SpatialState::navigate` fell back to `Direction::First`
 * whenever React sent a stale source key, then React's `focusedMoniker`
 * cleared because the winner's key had no React-side claim registration.
 * The first fix (task `01KPNWJKPH878XX66G421AS29G`) was to make navigate
 * a no-op on an unknown source.
 *
 * The "focus invariant" task (`01KPRGGCB5NYPW28AJZNM3D0QT`) replaced the
 * no-op with a real safety net: Rust now picks a successor automatically
 * on `unregister()`, and `navigate()` falls through to first-in-layer on
 * null or stale source. That closes the original bug from both sides:
 * React no longer gets a stale key (unregister emits a new focus) AND
 * even if it did, the fallback-to-first keeps the user navigating. This
 * file now locks down the new recovery behavior so future regressions
 * can't silently resurrect the wedge.
 *
 * Each scenario drives the real `EntityFocusProvider` + `FocusLayer` +
 * `FocusScope` tree through the JS `SpatialStateShim`. No production
 * code is bypassed.
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
   * Desync scenario: Rust silently loses the focused cell's spatial
   * entry without a `focus-changed` event. When the user presses `j`,
   * React sends Rust the now-stale key. Under the focus-invariant fix
   * Rust recognises the unknown source and falls through to
   * first-in-layer, emitting a `focus-changed` that restores React's
   * focused moniker onto a real cell.
   *
   * Before either fix: Rust jumped to first-in-layer but the winner
   * had no React claim, so React cleared its focused moniker and the
   * user was wedged.
   *
   * After the interim fix: Rust returned `Ok(None)`; cell00 stayed
   * visually focused but any real unregister flow (view swap, layer
   * pop) still produced a null focus that wedged later.
   *
   * After the invariant fix (this one): Rust picks the top-left entry
   * in the active layer (cell00 itself, since its React scope is still
   * mounted and re-registered on the shim's next rect write) — the
   * user never loses navigability. The assertion below just checks
   * that some cell in the grid retains focus, matching the invariant
   * contract regardless of which cell wins the tie-break.
   */
  it("pressing `j` after the focused key is dropped from Rust recovers onto a registered cell", async () => {
    const screen = await render(<AppWithGridFixture />);
    const cell00Mk = FIXTURE_CELL_MONIKERS[0][0];

    const cell00 = screen.getByTestId(`data-moniker:${cell00Mk}`);

    // 1) Click cell (0,0). React thinks focus is on cell00; Rust agrees.
    await userEvent.click(cell00);
    await expectFocused(cell00, "true");

    // 2) Capture the spatial key for cell (0,0) and silently delete it
    //    from Rust. This is the exact desync scenario: React's
    //    `monikerToKeysRef` still holds the key even though Rust has
    //    forgotten it.
    const staleKey = keyForMoniker(handles, cell00Mk);
    expect(staleKey).not.toBeNull();
    silentlyDropEntry(handles, staleKey!);

    // 3) Press `j` with the stale key. Under the focus-invariant fix
    //    Rust recognises the unknown source and picks the top-left
    //    entry in the active layer, emitting `focus-changed` so React
    //    updates its focused moniker too.
    await userEvent.keyboard("j");

    // Assert: the shim reports a non-null focused moniker. The
    // original "jumps-and-sticks" wedge was precisely that React's
    // focused moniker went null after the stale nav and subsequent
    // nav keys were silent no-ops. With the fallback-to-first safety
    // net the shim always moves to a real entry, so the user can keep
    // navigating from whichever cell Rust picks. We don't assert a
    // specific cell because the winner depends on the full set of
    // remaining rects (grid cells, row selectors, headers) — the
    // invariant is just that *something* is focused.
    await expect
      .poll(() => handles.focusedMoniker(), { timeout: FOCUS_POLL_TIMEOUT_MS })
      .toBeTruthy();
  });
});
