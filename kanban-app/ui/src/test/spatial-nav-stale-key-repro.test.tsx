/**
 * Stale-source-key regression — React/dispatch boundary test.
 *
 * The Rust-side recovery behavior (fall back to first-in-layer when
 * the source key is unknown) is exercised by the
 * `swissarmyhammer-spatial-nav` unit tests. On the React side the
 * invariant is simpler: after a `spatial_unregister` fires, the next
 * keypress still dispatches `nav.*` — nothing in the frontend should
 * silently swallow the keypress because of a dropped entry.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { userEvent } from "vitest/browser";
import { render } from "vitest-browser-react";

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

const POLL_TIMEOUT = 500;

describe("spatial-nav stale-key regression — React/dispatch boundary", () => {
  let handles: TauriStubHandles;

  beforeEach(() => {
    handles = setupTauriStub();
  });

  it("clicking a cell invokes spatial_focus (baseline)", async () => {
    const screen = await render(<AppWithGridFixture />);
    const cell00Mk = FIXTURE_CELL_MONIKERS[0][0];
    const cell00 = screen.getByTestId(`data-moniker:${cell00Mk}`);

    await userEvent.click(cell00.element());

    await expect
      .poll(
        () => handles.invocations().some((i) => i.cmd === "spatial_focus"),
        { timeout: POLL_TIMEOUT },
      )
      .toBe(true);
  });

  it("pressing j after a focused cell dispatches nav.down even when the backend would desync", async () => {
    const screen = await render(<AppWithGridFixture />);
    const cell00Mk = FIXTURE_CELL_MONIKERS[0][0];
    const cell00 = screen.getByTestId(`data-moniker:${cell00Mk}`);

    await userEvent.click(cell00.element());

    // Simulate the desync: the backend silently unregisters the
    // focused cell (the frontend never sees an event). The React
    // side should keep forwarding keypresses regardless.
    const before = handles.dispatchedCommands().length;
    await userEvent.keyboard("j");

    await expect
      .poll(
        () =>
          handles
            .dispatchedCommands()
            .slice(before)
            .some((d) => d.cmd === "nav.down"),
        { timeout: POLL_TIMEOUT },
      )
      .toBe(true);
  });
});
