/**
 * Inspector-layer navigation — React/dispatch boundary tests.
 *
 * Verifies the React wiring around `FocusLayer` push/pop for
 * inspectors: opening an inspector pushes a layer, closing it pops,
 * field scopes register on mount, and keypresses dispatch `nav.*`
 * through the command pipeline. The spatial algorithm that picks the
 * next field is owned by Rust.
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
  AppWithInspectorFixture,
  FIXTURE_CARD_MONIKER,
  FIXTURE_FIELD_MONIKERS,
} from "./spatial-inspector-fixture";

const POLL_TIMEOUT = 500;

describe("inspector field navigation — React/dispatch boundary", () => {
  let handles: TauriStubHandles;

  beforeEach(() => {
    handles = setupTauriStub();
  });

  it("double-click on the card pushes an inspector layer", async () => {
    const screen = await render(<AppWithInspectorFixture />);
    const card = screen.getByTestId("fixture-card");

    await userEvent.click(card.element());
    await userEvent.dblClick(card.element());

    // The inspector's FocusLayer mounts and pushes a layer via
    // `spatial_push_layer`.
    await expect
      .poll(
        () =>
          handles.invocations().filter((i) => i.cmd === "spatial_push_layer")
            .length,
        { timeout: POLL_TIMEOUT },
      )
      .toBeGreaterThan(0);
  });

  it("after inspector opens, field scopes register via spatial_register", async () => {
    const screen = await render(<AppWithInspectorFixture />);
    const card = screen.getByTestId("fixture-card");

    await userEvent.click(card.element());
    await userEvent.dblClick(card.element());

    for (const fieldMoniker of FIXTURE_FIELD_MONIKERS) {
      await expect
        .poll(
          () =>
            handles.invocations().some((i) => {
              if (i.cmd !== "spatial_register") return false;
              const a = i.args as { args?: { moniker?: string } };
              return a.args?.moniker === fieldMoniker;
            }),
          { timeout: POLL_TIMEOUT },
        )
        .toBe(true);
    }
  });

  it("pressing j inside the inspector dispatches nav.down", async () => {
    const screen = await render(<AppWithInspectorFixture />);
    const card = screen.getByTestId("fixture-card");

    await userEvent.click(card.element());
    await userEvent.dblClick(card.element());

    // Clear the dispatch log from any open-inspector side effects so
    // the assertion below isolates the keypress dispatch.
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

  it("Escape inside the inspector pops the layer", async () => {
    const screen = await render(<AppWithInspectorFixture />);
    const card = screen.getByTestId("fixture-card");

    await userEvent.click(card.element());
    await userEvent.dblClick(card.element());

    // Verify a push happened.
    await expect
      .poll(
        () =>
          handles.invocations().filter((i) => i.cmd === "spatial_push_layer")
            .length,
        { timeout: POLL_TIMEOUT },
      )
      .toBeGreaterThan(0);

    await userEvent.keyboard("{Escape}");

    await expect
      .poll(
        () =>
          handles.invocations().filter((i) => i.cmd === "spatial_remove_layer")
            .length,
        { timeout: POLL_TIMEOUT },
      )
      .toBeGreaterThan(0);
  });

  it("the card's scope registers via spatial_register on mount", async () => {
    await render(<AppWithInspectorFixture />);

    await expect
      .poll(
        () =>
          handles.invocations().some((i) => {
            if (i.cmd !== "spatial_register") return false;
            const a = i.args as { args?: { moniker?: string } };
            return a.args?.moniker === FIXTURE_CARD_MONIKER;
          }),
        { timeout: POLL_TIMEOUT },
      )
      .toBe(true);
  });
});
