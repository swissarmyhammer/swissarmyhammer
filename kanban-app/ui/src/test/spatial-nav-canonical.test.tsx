/**
 * Canonical vitest-browser test for spatial navigation at the
 * React/Tauri boundary.
 *
 * ## Purpose
 *
 * This file pins down the React-side half of the spatial-nav
 * round-trip:
 *
 * 1. Clicking a `FocusScope`-wrapped cell invokes
 *    `spatial_focus(key)` with the cell's key and flips its
 *    `data-focused` attribute once the backend emits `focus-changed`.
 * 2. Pressing `j` dispatches `nav.down` via `dispatch_command`. The
 *    stub then simulates the backend emitting `focus-changed` with
 *    the next cell's key and the next cell's `data-focused` flips.
 *
 * The algorithm that chooses "which cell is next" lives entirely in
 * Rust (`swissarmyhammer-spatial-nav`). Tests here do NOT compute
 * that answer — they script "the backend says next is cell (1,0)"
 * and assert the UI reacts.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { userEvent } from "vitest/browser";
import { render } from "vitest-browser-react";

// Wire the Tauri API mocks to the thin boundary stub. `vi.mock` calls
// must appear literally in this file so vitest's hoist catches them —
// the factories live in `setup-tauri-stub` so new tests can copy this
// boilerplate without duplicating the dispatcher logic.
vi.mock("@tauri-apps/api/core", async () => {
  const { tauriCoreMock } = await import("./setup-tauri-stub");
  return tauriCoreMock();
});
vi.mock("@tauri-apps/api/event", async () => {
  const { tauriEventMock } = await import("./setup-tauri-stub");
  return tauriEventMock();
});
vi.mock("@tauri-apps/api/window", async () => {
  const { tauriWindowMock } = await import("./setup-tauri-stub");
  return tauriWindowMock();
});
vi.mock("@tauri-apps/api/webviewWindow", async () => {
  const { tauriWebviewWindowMock } = await import("./setup-tauri-stub");
  return tauriWebviewWindowMock();
});
vi.mock("@tauri-apps/plugin-log", async () => {
  const { tauriPluginLogMock } = await import("./setup-tauri-stub");
  return tauriPluginLogMock();
});

import { setupTauriStub, type TauriStubHandles } from "./setup-tauri-stub";
import {
  AppWithGridFixture,
  FIXTURE_CELL_MONIKERS,
} from "./spatial-grid-fixture";

describe("spatial navigation — React/dispatch boundary (canonical)", () => {
  let handles: TauriStubHandles;

  beforeEach(() => {
    handles = setupTauriStub();
  });

  it("pressing 'j' from cell (0,0) dispatches nav.down and flips data-focused on (1,0)", async () => {
    const screen = await render(<AppWithGridFixture />);

    const cell00Moniker = FIXTURE_CELL_MONIKERS[0][0];
    const cell10Moniker = FIXTURE_CELL_MONIKERS[1][0];
    const cell00 = screen.getByTestId(`data-moniker:${cell00Moniker}`);
    const cell10 = screen.getByTestId(`data-moniker:${cell10Moniker}`);

    // Script the backend's response to nav.down: focus moves from
    // cell (0,0) to cell (1,0). This is the algorithm's concern; the
    // stub just declares what Rust would have decided.
    handles.scriptResponse("dispatch_command:nav.down", () =>
      handles.payloadForFocusMove(cell00Moniker, cell10Moniker),
    );

    await userEvent.click(cell00.element());
    // spatial_focus → focus-changed round-trip flips data-focused.
    await expect
      .poll(() => cell00.element().getAttribute("data-focused"), {
        timeout: 300,
      })
      .toBe("true");

    await userEvent.keyboard("j");
    await expect
      .poll(() => cell10.element().getAttribute("data-focused"), {
        timeout: 300,
      })
      .toBe("true");
    // The origin cell loses its decoration on the same event.
    expect(cell00.element().getAttribute("data-focused")).toBe(null);

    // And the React → Tauri boundary fired the expected command.
    expect(handles.dispatchedCommands().some((d) => d.cmd === "nav.down")).toBe(
      true,
    );
  });
});
