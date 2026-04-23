/**
 * Inspector-over-grid navigation — React/dispatch boundary tests.
 *
 * Verifies that with a dense grid in the window layer and an
 * inspector layer on top, the inspector's field scopes register
 * correctly and keypresses dispatch `nav.*` as expected. The
 * layer-isolation algorithm itself is covered by Rust unit tests.
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
  AppWithInspectorOverGridFixture,
  FIXTURE_FIELD_MONIKERS,
} from "./spatial-inspector-over-grid-fixture";

const POLL_TIMEOUT = 500;

describe("inspector-over-grid — React/dispatch boundary", () => {
  let handles: TauriStubHandles;

  beforeEach(() => {
    handles = setupTauriStub();
  });

  it("inspector's field scopes register even under a dense grid layer", async () => {
    const screen = await render(<AppWithInspectorOverGridFixture />);
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
    const screen = await render(<AppWithInspectorOverGridFixture />);
    const card = screen.getByTestId("fixture-card");

    await userEvent.click(card.element());
    await userEvent.dblClick(card.element());

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

  it("opening the inspector pushes exactly one additional layer", async () => {
    const screen = await render(<AppWithInspectorOverGridFixture />);
    const card = screen.getByTestId("fixture-card");

    const pushesBefore = handles
      .invocations()
      .filter((i) => i.cmd === "spatial_push_layer").length;

    await userEvent.click(card.element());
    await userEvent.dblClick(card.element());

    await expect
      .poll(
        () =>
          handles.invocations().filter((i) => i.cmd === "spatial_push_layer")
            .length,
        { timeout: POLL_TIMEOUT },
      )
      .toBe(pushesBefore + 1);
  });
});
