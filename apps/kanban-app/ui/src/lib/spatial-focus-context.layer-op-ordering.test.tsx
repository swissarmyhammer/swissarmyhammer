/**
 * Layer-op ordering: the kernel must observe push/pop layer ops in React
 * lifecycle order (kanban `01KTQCHWP5T4GS8SPGYVXD2CT9`).
 *
 * Live failure this pins: with two windows on the same board, the second
 * window rendered no focus markers. The unified log showed every one of its
 * focus commits dropped with "focus snapshot names an unregistered layer" —
 * its window-root layer (`/<label>/window`) was missing from the kernel
 * registry even though `<FocusLayer>` had pushed it.
 *
 * Mechanism: React StrictMode (dev) double-invokes effects, so the window
 * root's push effect fires push(fq) → cleanup pop(fq) → push(fq) as THREE
 * separate async MCP calls. Each call is handled as an independent task on
 * the host (contending on the per-board platform lock), so completion order
 * is not arrival order: when the cleanup pop is processed AFTER the remount
 * push, the layer is deleted permanently and nothing ever re-pushes it.
 * Window-unique layer FQs (`/<label>/window`) made this fatal — under the
 * old shared `/window` root the sibling window's surviving push masked the
 * lost one.
 *
 * Invariant: `SpatialFocusProvider` serializes kernel layer ops — a layer op
 * is dispatched only after the previous one has fully completed — so the
 * kernel registry ends in the state React lifecycle order implies.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { StrictMode } from "react";
import { render, act } from "@testing-library/react";

const mockInvoke = vi.fn(
  (..._args: unknown[]): Promise<unknown> => Promise.resolve(),
);

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

import { SpatialFocusProvider } from "./spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { asSegment } from "@/types/spatial";

/** Focus-tool layer op as seen on the `command_tool_call` wire. */
interface LayerOpCall {
  tool?: string;
  op?: string;
  params?: { fq?: string };
}

/** How long the shadow kernel takes to process a pop (ms). */
const SLOW_POP_MS = 20;

describe("SpatialFocusProvider layer-op ordering", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("window-root layer survives a StrictMode remount whose pop is processed slowly", async () => {
    // Shadow of the kernel's layer registry. Mutations are applied when the
    // host "processes" each op — pushes process immediately, pops process
    // SLOW_POP_MS after arrival — modelling the real host, where each MCP
    // call is an independent task and completion order is not arrival order.
    const layers = new Set<string>();
    let pushArrivals = 0;
    mockInvoke.mockImplementation((cmd: unknown, rawArgs?: unknown) => {
      const args = (rawArgs ?? {}) as LayerOpCall;
      if (cmd === "command_tool_call" && args.tool === "focus") {
        const fq = args.params?.fq;
        if (args.op === "push layer" && fq !== undefined) {
          pushArrivals += 1;
          layers.add(fq);
          return Promise.resolve({ ok: true });
        }
        if (args.op === "pop layer" && fq !== undefined) {
          return new Promise((resolve) => {
            setTimeout(() => {
              layers.delete(fq);
              resolve({ ok: true, next_fq: null });
            }, SLOW_POP_MS);
          });
        }
      }
      return Promise.resolve();
    });

    render(
      <StrictMode>
        <SpatialFocusProvider>
          <FocusLayer name={asSegment("window")}>{null}</FocusLayer>
        </SpatialFocusProvider>
      </StrictMode>,
    );

    // Let the StrictMode mount → cleanup → remount triple and the slow pop
    // fully settle.
    await act(async () => {
      await new Promise((r) => setTimeout(r, SLOW_POP_MS * 5));
    });

    // Guard: StrictMode actually exercised the remount path (two pushes).
    // Without it this test would vacuously pass on a single mount.
    expect(pushArrivals).toBe(2);

    // The invariant: after push → pop → push, the layer is REGISTERED. On
    // unserialized dispatch the slow pop lands after the remount push and
    // deletes it — the live "focus snapshot names an unregistered layer"
    // failure mode.
    expect([...layers]).toEqual(["/window"]);
  });
});
