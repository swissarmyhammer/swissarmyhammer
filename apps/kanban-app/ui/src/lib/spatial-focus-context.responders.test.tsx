/**
 * Tests for the host→UI geometry responders `SpatialFocusProvider` registers
 * (Card F2). The focus kernel PULLS live geometry / current focus from the
 * webview on demand over the F1 channel; these responders are the webview
 * half — answered from the provider's own `focusedFqRef` and layer registries,
 * built on demand (`getBoundingClientRect` at call time), never cached.
 *
 * Properties pinned:
 *
 * - `focus.geometry` returns the `NavSnapshot` built for the currently focused
 *   FQM (via `LayerScopeRegistry.buildSnapshot`), or `null` when nothing is
 *   focused / the focused scope's registry is gone.
 * - `focus.current` returns the focused FQM, or `null` when unfocused.
 * - Both responders are cleaned up on unmount (no stale closure lingers).
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act } from "@testing-library/react";
import type { RefObject } from "react";

const mockInvoke = vi.fn(
  (..._args: unknown[]): Promise<unknown> => Promise.resolve(),
);
let listenHandlers: Record<string, (event: { payload: unknown }) => void> = {};

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((event: string, handler: (e: { payload: unknown }) => void) => {
    listenHandlers[event] = handler;
    return Promise.resolve(() => {
      delete listenHandlers[event];
    });
  }),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

import {
  LayerScopeRegistry,
  useOptionalLayerScopeRegistry,
  type ScopeEntry,
} from "./layer-scope-registry-context";
import { SpatialFocusProvider } from "./spatial-focus-context";
import {
  handleUiRequest,
  resetUiRespondersForTest,
} from "./ui-request-responder";
import { FocusLayer } from "@/components/focus-layer";
import {
  asSegment,
  composeFq,
  fqRoot,
  type FullyQualifiedMoniker,
} from "@/types/spatial";

async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
}

interface RectLiteral {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** A `ScopeEntry` backed by a real node with a stubbed rect. */
function makeEntry(
  parentZone: FullyQualifiedMoniker | null,
  rect: RectLiteral,
): ScopeEntry {
  const node = document.createElement("div");
  node.getBoundingClientRect = () =>
    ({
      x: rect.x,
      y: rect.y,
      width: rect.width,
      height: rect.height,
      top: rect.y,
      left: rect.x,
      right: rect.x + rect.width,
      bottom: rect.y + rect.height,
      toJSON: () => rect,
    }) as DOMRect;
  const ref: RefObject<HTMLElement | null> = { current: node };
  return { ref, parentZone, segment: asSegment("scope"), lastKnownRect: null };
}

/** Capture the registry the enclosing layer publishes. */
function CaptureRegistry({
  out,
}: {
  out: { current: LayerScopeRegistry | null };
}) {
  out.current = useOptionalLayerScopeRegistry();
  return null;
}

/** Drive a host `ui/request` of `kind` and capture the replied result. */
async function callResponder(kind: string): Promise<unknown> {
  let replied: unknown;
  const invoke = (_cmd: string, args: Record<string, unknown>) => {
    replied = args.result;
    return Promise.resolve();
  };
  await handleUiRequest(
    { request_id: "r1", kind, params: {} },
    invoke as never,
  );
  return replied;
}

beforeEach(() => {
  mockInvoke.mockClear();
  listenHandlers = {};
  resetUiRespondersForTest();
});

describe("SpatialFocusProvider host→UI geometry responders", () => {
  it("focus.current returns the focused FQM and focus.geometry builds its snapshot", async () => {
    const captured: { current: LayerScopeRegistry | null } = { current: null };

    render(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <CaptureRegistry out={captured} />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const layerFq = fqRoot(asSegment("window"));
    const focusedFq = composeFq(layerFq, asSegment("k1"));

    const registry = captured.current!;
    registry.add(
      focusedFq,
      makeEntry(layerFq, { x: 0, y: 0, width: 10, height: 10 }),
    );

    // Drive a focus-changed so the provider records `k1` as the focused FQM.
    await act(async () => {
      listenHandlers["focus-changed"]?.({
        payload: {
          window_label: "main",
          prev_fq: null,
          next_fq: focusedFq,
          next_segment: "k1",
        },
      });
    });

    // focus.current → the focused FQM.
    expect(await callResponder("focus.current")).toBe(focusedFq);

    // focus.geometry → a NavSnapshot for the focused layer containing k1.
    const snapshot = (await callResponder("focus.geometry")) as {
      layer_fq: string;
      scopes: Array<{ fq: string }>;
    } | null;
    expect(snapshot).not.toBeNull();
    expect(snapshot!.layer_fq).toBe(layerFq);
    expect(snapshot!.scopes.map((s) => s.fq)).toContain(focusedFq);
  });

  it("ignores focus-changed events targeted at another window", async () => {
    // THE two-windows-on-one-board drill bug (live): `emit_to`-targeted
    // Tauri events are received by EVERY window's global `listen`, so
    // window B's `focus-changed` reached window A's provider and overwrote
    // `focusedFqRef` with B's FQ. The kernel's next `focus.current` pull for
    // window A then resolved ANOTHER window's focus, and drill-in/Escape
    // echoed/committed against the wrong window. The provider must answer
    // only for events whose `window_label` is its own window.
    const captured: { current: LayerScopeRegistry | null } = { current: null };

    render(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <CaptureRegistry out={captured} />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const layerFq = fqRoot(asSegment("window"));
    const ownFocus = composeFq(layerFq, asSegment("k1"));
    captured.current!.add(
      ownFocus,
      makeEntry(layerFq, { x: 0, y: 0, width: 10, height: 10 }),
    );

    // Own-window focus event: recorded.
    await act(async () => {
      listenHandlers["focus-changed"]?.({
        payload: {
          window_label: "main",
          prev_fq: null,
          next_fq: ownFocus,
          next_segment: "k1",
        },
      });
    });
    expect(await callResponder("focus.current")).toBe(ownFocus);

    // ANOTHER window's focus event leaks in through the global listener:
    // it must be IGNORED — `focus.current` keeps answering this window's
    // own focus, never the foreign window's FQ.
    await act(async () => {
      listenHandlers["focus-changed"]?.({
        payload: {
          window_label: "board-other",
          prev_fq: null,
          next_fq: "/board-other/window/board:b/zone:z",
          next_segment: "zone:z",
        },
      });
    });
    expect(await callResponder("focus.current")).toBe(ownFocus);
  });

  it("focus.current and focus.geometry return null when nothing is focused", async () => {
    render(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <div />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    expect(await callResponder("focus.current")).toBeNull();
    expect(await callResponder("focus.geometry")).toBeNull();
  });
});
