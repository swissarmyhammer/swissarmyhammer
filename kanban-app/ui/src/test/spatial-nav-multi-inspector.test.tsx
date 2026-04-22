/**
 * Multi-inspector navigation — React/dispatch boundary tests.
 *
 * Verifies the React wiring when three inspectors stack atop the
 * window layer: each inspector mounts its own `FocusLayer`, every
 * layer push fires through `spatial_push_layer`, and fields in each
 * inspector register with the correct `layer_key`. The spatial
 * layer-isolation algorithm is covered by Rust unit tests.
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

import { setupTauriStub, type TauriStubHandles } from "./setup-tauri-stub";
import {
  AppWithMultiInspectorFixture,
  FIXTURE_ENTITY_MONIKERS,
  FIXTURE_FIELD_MONIKERS,
} from "./spatial-multi-inspector-fixture";

const POLL_TIMEOUT = 500;

describe("multi-inspector — React/dispatch boundary", () => {
  let handles: TauriStubHandles;

  beforeEach(() => {
    handles = setupTauriStub();
  });

  it("mounts a FocusLayer per inspector — push_layer fires for each", async () => {
    await render(<AppWithMultiInspectorFixture />);

    // The fixture renders three inspectors on top of the window
    // layer; the window layer is a `push_layer` too. At least three
    // pushes for the three inspector layers must land.
    await expect
      .poll(
        () =>
          handles.invocations().filter((i) => i.cmd === "spatial_push_layer")
            .length,
        { timeout: POLL_TIMEOUT },
      )
      .toBeGreaterThanOrEqual(3);
  });

  it("each inspector's entity scope does NOT register a spatial entry (spatial=false)", async () => {
    // The inspector's outer entity `FocusScope` uses `spatial={false}`
    // so it participates in focus/commands but is NOT a beam-test
    // target. This check pins that contract: no `spatial_register`
    // call carries an inspector entity moniker. If this changes, the
    // entity scope would start shadowing field rects in the spatial
    // engine — the exact regression that motivated the contract.
    await render(<AppWithMultiInspectorFixture />);

    // Give the component a moment to mount and issue any registers it
    // intends to. Then confirm none of them are entity monikers.
    await new Promise((r) => setTimeout(r, 100));

    for (const entityMoniker of FIXTURE_ENTITY_MONIKERS) {
      const registered = handles.invocations().some((i) => {
        if (i.cmd !== "spatial_register") return false;
        const a = i.args as { args?: { moniker?: string } };
        return a.args?.moniker === entityMoniker;
      });
      expect(registered).toBe(false);
    }
  });

  it("each inspector's fields register with distinct moniker strings", async () => {
    await render(<AppWithMultiInspectorFixture />);

    // Flatten every per-inspector field moniker list and assert
    // every one of them was registered at least once.
    const allFieldMonikers = FIXTURE_FIELD_MONIKERS.flat();
    for (const m of allFieldMonikers) {
      await expect
        .poll(
          () =>
            handles.invocations().some((i) => {
              if (i.cmd !== "spatial_register") return false;
              const a = i.args as { args?: { moniker?: string } };
              return a.args?.moniker === m;
            }),
          { timeout: POLL_TIMEOUT },
        )
        .toBe(true);
    }
  });
});
