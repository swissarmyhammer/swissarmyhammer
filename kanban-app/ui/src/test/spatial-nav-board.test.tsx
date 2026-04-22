/**
 * Board-view spatial navigation — React/dispatch boundary tests.
 *
 * Asserts the React half of the h/j/k/l contract: keypresses flow
 * through `dispatch_command` and scripted backend responses flip
 * `data-focused` on the target card. The algorithm that picks the
 * next card is Rust's concern and is covered by
 * `swissarmyhammer-spatial-nav` unit tests.
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
  AppWithBoardFixture,
  FIXTURE_CARD_MONIKERS,
} from "./spatial-board-fixture";

const FOCUS_POLL_TIMEOUT = 500;

async function expectFocused(el: HTMLElement): Promise<void> {
  await expect
    .poll(() => el.getAttribute("data-focused"), {
      timeout: FOCUS_POLL_TIMEOUT,
    })
    .toBe("true");
}

describe("board card navigation — React/dispatch boundary", () => {
  let handles: TauriStubHandles;

  beforeEach(() => {
    handles = setupTauriStub();
  });

  it("click on a card dispatches spatial_focus and flips data-focused", async () => {
    const screen = await render(<AppWithBoardFixture />);
    const card11 = screen
      .getByTestId(`data-moniker:${FIXTURE_CARD_MONIKERS[0][0]}`)
      .element() as HTMLElement;

    await userEvent.click(card11);
    await expectFocused(card11);

    // The click must have invoked `spatial_focus` with a key — the
    // React → Tauri contract the stub records.
    const focusCalls = handles
      .invocations()
      .filter((i) => i.cmd === "spatial_focus");
    expect(focusCalls.length).toBeGreaterThan(0);
  });

  it("pressing j dispatches nav.down and scripted response flips the next card", async () => {
    const screen = await render(<AppWithBoardFixture />);
    const card11Mk = FIXTURE_CARD_MONIKERS[0][0];
    const card12Mk = FIXTURE_CARD_MONIKERS[0][1];
    const card11 = screen
      .getByTestId(`data-moniker:${card11Mk}`)
      .element() as HTMLElement;
    const card12 = screen
      .getByTestId(`data-moniker:${card12Mk}`)
      .element() as HTMLElement;

    handles.scriptResponse("dispatch_command:nav.down", () =>
      handles.payloadForFocusMove(card11Mk, card12Mk),
    );

    await userEvent.click(card11);
    await expectFocused(card11);

    await userEvent.keyboard("j");
    await expectFocused(card12);
    expect(handles.dispatchedCommands().some((d) => d.cmd === "nav.down")).toBe(
      true,
    );
  });

  it("pressing l dispatches nav.right and scripted response flips the next card", async () => {
    const screen = await render(<AppWithBoardFixture />);
    const card12Mk = FIXTURE_CARD_MONIKERS[0][1];
    const card22Mk = FIXTURE_CARD_MONIKERS[1][1];
    const card12 = screen
      .getByTestId(`data-moniker:${card12Mk}`)
      .element() as HTMLElement;
    const card22 = screen
      .getByTestId(`data-moniker:${card22Mk}`)
      .element() as HTMLElement;

    handles.scriptResponse("dispatch_command:nav.right", () =>
      handles.payloadForFocusMove(card12Mk, card22Mk),
    );

    await userEvent.click(card12);
    await expectFocused(card12);

    await userEvent.keyboard("l");
    await expectFocused(card22);
    expect(
      handles.dispatchedCommands().some((d) => d.cmd === "nav.right"),
    ).toBe(true);
  });

  it("pressing j with no scripted response (backend clamped) keeps focus put", async () => {
    const screen = await render(<AppWithBoardFixture />);
    const cardBottomMk = FIXTURE_CARD_MONIKERS[1][2];
    const cardBottom = screen
      .getByTestId(`data-moniker:${cardBottomMk}`)
      .element() as HTMLElement;

    // No scriptResponse for nav.down — the stub emits no
    // focus-changed event, modeling a clamp at the bottom row.

    await userEvent.click(cardBottom);
    await expectFocused(cardBottom);

    await userEvent.keyboard("j");

    // Dispatch fired …
    expect(handles.dispatchedCommands().some((d) => d.cmd === "nav.down")).toBe(
      true,
    );
    // … but focus stayed put because no focus-changed event landed.
    expect(cardBottom.getAttribute("data-focused")).toBe("true");
  });
});
