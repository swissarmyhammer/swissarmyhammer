/**
 * Grid cell-to-cell navigation — React/dispatch boundary tests.
 *
 * Asserts the React-wiring contract for the DataTable-like fixture:
 * clicking a cell dispatches `spatial_focus`, keypresses dispatch
 * `nav.*`, and scripted backend responses flip `data-focused` on the
 * intended cell. The spatial algorithm itself is Rust's concern.
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
  FIXTURE_COLUMN_HEADER_MONIKERS,
  FIXTURE_ROW_SELECTOR_MONIKERS,
} from "./spatial-grid-fixture";

const FOCUS_POLL_TIMEOUT_MS = 300;

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

describe("grid cell navigation — React/dispatch boundary", () => {
  let handles: TauriStubHandles;

  beforeEach(() => {
    handles = setupTauriStub();
  });

  it("click on a cell invokes spatial_focus and flips data-focused", async () => {
    const screen = await render(<AppWithGridFixture />);
    const cell00 = screen.getByTestId(
      `data-moniker:${FIXTURE_CELL_MONIKERS[0][0]}`,
    );

    await userEvent.click(cell00.element());
    await expectFocused(cell00, "true");

    expect(handles.invocations().some((i) => i.cmd === "spatial_focus")).toBe(
      true,
    );
  });

  it("h/j/k/l keypresses each fire dispatch_command with the matching nav.* id", async () => {
    const screen = await render(<AppWithGridFixture />);
    const cell00 = screen.getByTestId(
      `data-moniker:${FIXTURE_CELL_MONIKERS[0][0]}`,
    );

    await userEvent.click(cell00.element());
    await expectFocused(cell00, "true");

    await userEvent.keyboard("j");
    await userEvent.keyboard("l");
    await userEvent.keyboard("k");
    await userEvent.keyboard("h");

    const ids = handles.dispatchedCommands().map((d) => d.cmd);
    expect(ids).toContain("nav.down");
    expect(ids).toContain("nav.right");
    expect(ids).toContain("nav.up");
    expect(ids).toContain("nav.left");
  });

  it("scripted backend response on nav.down flips data-focused from (0,0) to (1,0)", async () => {
    const screen = await render(<AppWithGridFixture />);
    const cell00Mk = FIXTURE_CELL_MONIKERS[0][0];
    const cell10Mk = FIXTURE_CELL_MONIKERS[1][0];
    const cell00 = screen.getByTestId(`data-moniker:${cell00Mk}`);
    const cell10 = screen.getByTestId(`data-moniker:${cell10Mk}`);

    handles.scriptResponse("dispatch_command:nav.down", () =>
      handles.payloadForFocusMove(cell00Mk, cell10Mk),
    );

    await userEvent.click(cell00.element());
    await expectFocused(cell00, "true");

    await userEvent.keyboard("j");
    await expectFocused(cell10, "true");
    await expectFocused(cell00, null);
  });

  it("row selector registers its moniker as a spatial entry", async () => {
    // React-boundary check: the row selector FocusScope must invoke
    // spatial_register with its moniker, otherwise Rust has no rect
    // for it and no amount of algorithm work will move focus there.
    const screen = await render(<AppWithGridFixture />);

    const selectorMoniker = FIXTURE_ROW_SELECTOR_MONIKERS[2];
    // Poll until the ResizeObserver flush inside FocusScope has
    // invoked spatial_register at least once for this moniker.
    await expect
      .poll(
        () =>
          handles.invocations().some((i) => {
            if (i.cmd !== "spatial_register") return false;
            const a = i.args as { args?: { moniker?: string } };
            return a.args?.moniker === selectorMoniker;
          }),
        { timeout: FOCUS_POLL_TIMEOUT_MS },
      )
      .toBe(true);

    // Sanity: the selector is also present in the DOM.
    const selectorEl = screen.getByTestId(`data-moniker:${selectorMoniker}`);
    expect(selectorEl.element()).toBeTruthy();
  });

  it("column headers register their monikers as spatial entries", async () => {
    await render(<AppWithGridFixture />);

    for (const hdrMoniker of FIXTURE_COLUMN_HEADER_MONIKERS) {
      await expect
        .poll(
          () =>
            handles.invocations().some((i) => {
              if (i.cmd !== "spatial_register") return false;
              const a = i.args as { args?: { moniker?: string } };
              return a.args?.moniker === hdrMoniker;
            }),
          { timeout: FOCUS_POLL_TIMEOUT_MS },
        )
        .toBe(true);
    }
  });
});
