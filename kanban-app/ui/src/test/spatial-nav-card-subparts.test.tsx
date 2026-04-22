/**
 * Card sub-part navigation — React/dispatch boundary tests.
 *
 * Verifies the React wiring for card interior scopes: each tag pill
 * registers with the enclosing card's moniker as `parent_scope` so
 * Rust can run the container-first search. The spatial selection
 * itself lives in Rust.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
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

import { moniker } from "@/lib/moniker";
import { setupTauriStub, type TauriStubHandles } from "./setup-tauri-stub";
import {
  AppWithBoardFixture,
  FIXTURE_CARD_MONIKERS,
  fixtureTagId,
} from "./spatial-board-fixture";

const POLL_TIMEOUT = 500;

/** Build the moniker for the Nth tag pill on the (col, row) card. */
function tagMoniker(col: number, row: number, idx: number): string {
  return moniker("tag", fixtureTagId(col, row, idx));
}

describe("card sub-parts — React/dispatch boundary", () => {
  let handles: TauriStubHandles;

  beforeEach(() => {
    handles = setupTauriStub();
  });

  it("pill spatial entries carry the enclosing card moniker as parent_scope", async () => {
    await render(<AppWithBoardFixture />);

    const cardMoniker = FIXTURE_CARD_MONIKERS[0][0];
    const pill1Moniker = tagMoniker(1, 1, 1);
    const pill2Moniker = tagMoniker(1, 1, 2);

    // Poll until both pills are registered.
    await expect
      .poll(
        () =>
          handles.invocations().filter((i) => {
            if (i.cmd !== "spatial_register") return false;
            const a = i.args as { args?: { moniker?: string } };
            return (
              a.args?.moniker === pill1Moniker ||
              a.args?.moniker === pill2Moniker
            );
          }).length,
        { timeout: POLL_TIMEOUT },
      )
      .toBeGreaterThanOrEqual(2);

    // Each pill must carry the enclosing card's moniker as parent_scope.
    // Production's FocusScope accepts both `parentScope` and
    // `parent_scope` keys; check both for forward-compat.
    for (const pillMoniker of [pill1Moniker, pill2Moniker]) {
      const registers = handles.invocations().filter((i) => {
        if (i.cmd !== "spatial_register") return false;
        const a = i.args as { args?: { moniker?: string } };
        return a.args?.moniker === pillMoniker;
      });
      expect(registers.length).toBeGreaterThan(0);
      for (const reg of registers) {
        const args = (reg.args as { args: Record<string, unknown> }).args;
        const parent =
          (args.parentScope as string | null | undefined) ??
          (args.parent_scope as string | null | undefined);
        expect(parent).toBe(cardMoniker);
      }
    }
  });
});
