/**
 * Canonical vitest-browser test for spatial navigation, as specified by the
 * test-harness kanban task.
 *
 * This file is the first failing test in the TDD ladder for cell-level
 * spatial navigation. Per the task description:
 *
 * > MUST fail against HEAD today because cells aren't FocusScopes yet.
 *
 * When a sibling task wraps each DataTable cell in a `FocusScope`, this
 * test flips from red to green — demonstrating that the harness itself
 * is not the bottleneck; the production grid just needs to register
 * cells as spatial entries.
 *
 * ## What the test exercises
 *
 * 1. **React render**: real `EntityFocusProvider` + `FocusLayer` +
 *    `CommandScopeProvider` tree.
 * 2. **Click → focus**: `userEvent.click()` drives a real DOM click,
 *    which bubbles to `FocusScope`'s click handler and calls
 *    `invoke("spatial_focus", { key })`. The shim handles this
 *    synchronously, emits `focus-changed`, and the claim callback
 *    flips `data-focused="true"` on the focused scope.
 * 3. **Key → nav**: `userEvent.keyboard("j")` dispatches a real
 *    keydown. The production `createKeyHandler` resolves `j` →
 *    `nav.down` (vim mode), which calls `broadcastNavCommand("nav.down")`
 *    which `invoke`s `spatial_navigate`. The shim runs the beam test
 *    and emits a second `focus-changed` event.
 *
 * ## Why the test fails today
 *
 * Cells in `spatial-grid-fixture.tsx` are plain `<div>`s, not
 * `FocusScope`s, matching production's `DataTableRow` where only the
 * row is a `FocusScope`. So:
 *
 * - Clicking a cell bubbles to the *row* `FocusScope`, which focuses
 *   the row's entity moniker (`tag:tag-0`), not the cell's field moniker.
 * - `data-focused="true"` lands on the row, not the cell — so the
 *   first assertion on the cell's `data-focused` times out.
 *
 * When cells become `FocusScope`s, the click focuses the cell's field
 * moniker, `j` triggers `nav.down`, the shim picks the cell below by
 * beam test, and both assertions pass.
 *
 * ## Infrastructure this test proves out for sibling tasks
 *
 * Any future spatial-nav scenario test follows the same pattern:
 * `setupSpatialShim()` → `render(fixture)` → DOM interaction →
 * assert on `data-focused` and/or the shim state. No WebDriver, no
 * tauri-driver, no external processes.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { userEvent } from "vitest/browser";
import { render } from "vitest-browser-react";

// Wire the Tauri API mocks into the shim dispatcher. `vi.mock` calls
// must appear literally in this file so vitest's hoist catches them —
// the factories live in `setup-spatial-shim` so new tests can copy
// this boilerplate without duplicating the dispatcher logic.
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

import { setupSpatialShim } from "./setup-spatial-shim";
import {
  AppWithGridFixture,
  FIXTURE_CELL_MONIKERS,
} from "./spatial-grid-fixture";

describe("spatial navigation — canonical j test", () => {
  beforeEach(() => {
    setupSpatialShim();
  });

  // Originally authored as `it.fails` — at the time, cells in the
  // fixture were plain `<div>`s and the canonical `j` test was RED.
  // With cell-level `FocusScope`s now in place (see
  // `spatial-nav-grid.test.tsx` for the full h/j/k/l coverage), this
  // test flipped to green and must run as a plain `it`; leaving
  // `.fails` would make vitest error with "test passed unexpectedly".
  it("grid: pressing 'j' from cell (0,0) moves focus to cell (1,0)", async () => {
    const screen = await render(<AppWithGridFixture />);

    const cell00Moniker = FIXTURE_CELL_MONIKERS[0][0]; // field:tag:tag-0.tag_name
    const cell10Moniker = FIXTURE_CELL_MONIKERS[1][0]; // field:tag:tag-1.tag_name

    const cell00 = screen.getByTestId(`data-moniker:${cell00Moniker}`);
    const cell10 = screen.getByTestId(`data-moniker:${cell10Moniker}`);

    await userEvent.click(cell00);
    // Use `expect.poll` with a tight (200ms) timeout — the shim is
    // synchronous and React renders flush well under this budget when
    // cells ARE `FocusScope`s. When they aren't, the attribute never
    // appears and the poll fails quickly instead of waiting for the
    // default multi-second timeout to elapse. Keeps this red test from
    // slowing CI.
    await expect
      .poll(() => cell00.element().getAttribute("data-focused"), {
        timeout: 200,
      })
      .toBe("true");

    await userEvent.keyboard("j");
    await expect
      .poll(() => cell10.element().getAttribute("data-focused"), {
        timeout: 200,
      })
      .toBe("true");
  });
});
