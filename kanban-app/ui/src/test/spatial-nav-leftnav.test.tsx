/**
 * LeftNav spatial-nav coverage — React/dispatch boundary tests.
 *
 * Asserts the React wiring: each LeftNav button is wrapped in a
 * `FocusScope` with its `view:*` moniker, keypresses dispatch
 * `nav.*`, Enter on a focused button dispatches `view.switch:<id>`,
 * and the Tauri boundary receives the expected commands. The spatial
 * algorithm is covered by Rust unit tests.
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

vi.mock("@/lib/views-context", () => {
  const views = [
    { id: "board", name: "Board", kind: "board", icon: "kanban" },
    { id: "grid", name: "Grid", kind: "grid", icon: "table" },
  ] as const;
  return {
    ViewsProvider: ({ children }: { children: React.ReactNode }) => children,
    useViews: () => ({
      views,
      activeView: views[0],
      setActiveViewId: vi.fn(),
      refresh: vi.fn(),
    }),
  };
});

import { setupTauriStub, type TauriStubHandles } from "./setup-tauri-stub";
import {
  AppWithBoardAndLeftNavFixture,
  FIXTURE_CARD_MONIKERS,
  FIXTURE_VIEW_MONIKERS,
} from "./spatial-leftnav-fixture";

const POLL_TIMEOUT = 500;

describe("LeftNav — React/dispatch boundary", () => {
  let handles: TauriStubHandles;

  beforeEach(() => {
    handles = setupTauriStub();
  });

  it("every view button registers its moniker as a spatial entry", async () => {
    await render(<AppWithBoardAndLeftNavFixture />);

    for (const viewMoniker of FIXTURE_VIEW_MONIKERS) {
      await expect
        .poll(
          () =>
            handles.invocations().some((i) => {
              if (i.cmd !== "spatial_register") return false;
              const a = i.args as { args?: { moniker?: string } };
              return a.args?.moniker === viewMoniker;
            }),
          { timeout: POLL_TIMEOUT },
        )
        .toBe(true);
    }
  });

  it("click on a view button invokes spatial_focus and dispatches view.switch", async () => {
    const screen = await render(<AppWithBoardAndLeftNavFixture />);
    const gridButton = screen
      .getByTestId(`data-moniker:${FIXTURE_VIEW_MONIKERS[1]}`)
      .element() as HTMLElement;

    await userEvent.click(gridButton);

    await expect
      .poll(
        () => handles.invocations().some((i) => i.cmd === "spatial_focus"),
        { timeout: POLL_TIMEOUT },
      )
      .toBe(true);

    await expect
      .poll(
        () =>
          handles
            .dispatchedCommands()
            .some((d) => d.cmd === "view.switch:grid"),
        { timeout: POLL_TIMEOUT },
      )
      .toBe(true);
  });

  it("Enter on a focused view button re-dispatches view.switch", async () => {
    const screen = await render(<AppWithBoardAndLeftNavFixture />);
    const gridButton = screen
      .getByTestId(`data-moniker:${FIXTURE_VIEW_MONIKERS[1]}`)
      .element() as HTMLElement;

    await userEvent.click(gridButton);

    // Wait for the click-driven dispatch to land so we can measure
    // "only new dispatches after Enter".
    await expect
      .poll(
        () =>
          handles
            .dispatchedCommands()
            .some((d) => d.cmd === "view.switch:grid"),
        { timeout: POLL_TIMEOUT },
      )
      .toBe(true);
    const afterClickCount = handles.dispatchedCommands().length;

    await userEvent.keyboard("{Enter}");

    await expect
      .poll(() => handles.dispatchedCommands().length, {
        timeout: POLL_TIMEOUT,
      })
      .toBeGreaterThan(afterClickCount);
    const enterDispatches = handles.dispatchedCommands().slice(afterClickCount);
    expect(enterDispatches.some((d) => d.cmd === "view.switch:grid")).toBe(
      true,
    );
  });

  it("h from a leftmost card dispatches nav.left", async () => {
    const screen = await render(<AppWithBoardAndLeftNavFixture />);
    const card11 = screen
      .getByTestId(`data-moniker:${FIXTURE_CARD_MONIKERS[0][0]}`)
      .element() as HTMLElement;

    await userEvent.click(card11);
    const before = handles.dispatchedCommands().length;
    await userEvent.keyboard("h");

    await expect
      .poll(
        () =>
          handles
            .dispatchedCommands()
            .slice(before)
            .some((d) => d.cmd === "nav.left"),
        { timeout: POLL_TIMEOUT },
      )
      .toBe(true);
  });

  it("scripted focus-changed moves focus across the left edge", async () => {
    const screen = await render(<AppWithBoardAndLeftNavFixture />);
    const cardMk = FIXTURE_CARD_MONIKERS[0][0];
    const viewMk = FIXTURE_VIEW_MONIKERS[0];
    const card = screen
      .getByTestId(`data-moniker:${cardMk}`)
      .element() as HTMLElement;
    const viewButton = screen
      .getByTestId(`data-moniker:${viewMk}`)
      .element() as HTMLElement;

    handles.scriptResponse("dispatch_command:nav.left", () =>
      handles.payloadForFocusMove(cardMk, viewMk),
    );

    await userEvent.click(card);
    await expect
      .poll(() => card.getAttribute("data-focused"), { timeout: POLL_TIMEOUT })
      .toBe("true");

    await userEvent.keyboard("h");
    await expect
      .poll(() => viewButton.getAttribute("data-focused"), {
        timeout: POLL_TIMEOUT,
      })
      .toBe("true");
  });
});
