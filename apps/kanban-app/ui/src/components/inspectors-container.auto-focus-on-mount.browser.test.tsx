/**
 * Browser test for card `01KR7CDEFWWVF4WH0BCHE8Y21J` step 6:
 * auto-focus on inspector layer mount via `nav.focus`.
 *
 * When the inspector layer mounts (panel stack 0 → 1), the
 * `useFirstFieldFocus` hook in `<EntityInspector>` dispatches
 * `nav.focus` with the first field's FQM. The dispatch is deferred
 * one tick (`queueMicrotask`) so the layer's own push effect fires
 * first, ensuring the kernel sees the layer registered before the
 * focus claim arrives.
 *
 * This test pins the contract directly: when an inspector-shaped
 * `<FocusLayer>` mounts containing a first-field `<FocusScope>`, a
 * `nav.focus` dispatch fires for that field's FQM via
 * `spatial_focus(fq)` IPC. The dispatched FQM lives under
 * `/window/inspector/...`.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, waitFor } from "@testing-library/react";
import * as React from "react";

// ---------------------------------------------------------------------------
// Hoisted Tauri-API mocks.
// ---------------------------------------------------------------------------

type ListenCallback = (event: { payload: unknown }) => void;

const { mockInvoke, mockListen, listeners } = vi.hoisted(() => {
  const listeners = new Map<string, ListenCallback[]>();
  const mockInvoke = vi.fn(
    async (_cmd: string, _args?: unknown): Promise<unknown> => undefined,
  );
  const mockListen = vi.fn(
    (eventName: string, cb: ListenCallback): Promise<() => void> => {
      const cbs = listeners.get(eventName) ?? [];
      cbs.push(cb);
      listeners.set(eventName, cbs);
      return Promise.resolve(() => {
        const arr = listeners.get(eventName);
        if (arr) {
          const idx = arr.indexOf(cb);
          if (idx >= 0) arr.splice(idx, 1);
        }
      });
    },
  );
  return { mockInvoke, mockListen, listeners };
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...a: unknown[]) => mockInvoke(...(a as [string, unknown?])),
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(() => Promise.resolve()),
  listen: (...a: Parameters<typeof mockListen>) => mockListen(...a),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: vi.fn(() => Promise.resolve(() => {})),
  }),
}));

vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

// ---------------------------------------------------------------------------
// Imports — after mocks.
// ---------------------------------------------------------------------------

import { FocusLayer } from "./focus-layer";
import { FocusScope } from "./focus-scope";
import { useEnclosingLayerFq } from "./layer-fq-context";
import { useFullyQualifiedMoniker } from "@/components/fully-qualified-moniker-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider, useFocusedFq } from "@/lib/entity-focus-context";
import { useDispatchCommand } from "@/lib/command-scope";
import {
  asSegment,
  composeFq,
  type FullyQualifiedMoniker,
} from "@/types/spatial";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Collect every `spatial_focus` invocation, in order. */
function spatialFocusCalls(): FullyQualifiedMoniker[] {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_focus")
    .map((c) => (c[1] as { fq: FullyQualifiedMoniker }).fq);
}

/**
 * Reproduces the contract of `useFirstFieldFocus` in `entity-inspector.tsx`:
 * on first mount, dispatch `nav.focus` with the first field's FQM,
 * deferred via `queueMicrotask` so the surrounding inspector layer's
 * push effect fires first.
 *
 * Pulled into the test rather than reusing the production hook so the
 * test exercises the exact contract independently of `EntityInspector`'s
 * schema-driven first-field resolution. The behavior under test is
 * "auto-focus on inspector mount via nav.focus dispatch", which is
 * what this hook does.
 */
function useFirstFieldFocusProbe(firstFieldFq: FullyQualifiedMoniker) {
  const dispatchNavFocus = useDispatchCommand("nav.focus");
  const dispatchRef = React.useRef(dispatchNavFocus);
  dispatchRef.current = dispatchNavFocus;
  React.useEffect(() => {
    let cancelled = false;
    queueMicrotask(() => {
      if (cancelled) return;
      void dispatchRef.current({ args: { fq: firstFieldFq } }).catch(() => {});
    });
    return () => {
      cancelled = true;
    };
  }, [firstFieldFq]);
}

/**
 * Simulates an inspector layer + first field. Inside an inspector layer,
 * a single `<FocusScope moniker="field:task:T1.title">` represents the
 * first field; the embedded probe hook fires a `nav.focus` dispatch on
 * mount.
 */
function FauxInspectorLayer({
  firstFieldSegment,
}: {
  firstFieldSegment: string;
}) {
  const windowLayerFq = useEnclosingLayerFq();
  return (
    <FocusLayer name={asSegment("inspector")} parentLayerFq={windowLayerFq}>
      <FauxInspectorBody firstFieldSegment={firstFieldSegment} />
    </FocusLayer>
  );
}

function FauxInspectorBody({
  firstFieldSegment,
}: {
  firstFieldSegment: string;
}) {
  const inspectorLayerFq = useFullyQualifiedMoniker();
  const firstFieldFq = composeFq(
    inspectorLayerFq,
    asSegment(firstFieldSegment),
  );
  useFirstFieldFocusProbe(firstFieldFq);
  return (
    <>
      <FocusScope moniker={asSegment(firstFieldSegment)}>
        <span data-testid="first-field">first field</span>
      </FocusScope>
      <FocusScope moniker={asSegment("field:task:T1.body")}>
        <span data-testid="second-field">second field</span>
      </FocusScope>
    </>
  );
}

/**
 * Probe that renders the entity-focus store's broad-subscribed FQM
 * into a `data-testid="focused-probe"` text node. Re-renders on every
 * focus move (broad subscription via `useFocusedFq`), so a `waitFor`
 * on its text content is the natural way to assert the post-bridge
 * store state.
 */
function FocusedFqProbe() {
  const fq = useFocusedFq();
  return <span data-testid="focused-probe">{fq ?? "null"}</span>;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("InspectorsContainer — auto-focus on mount", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
  });

  it("on inspector layer mount, nav.focus dispatches with the first field's FQM (under /window/inspector/...)", async () => {
    // Stub IPC: forward `focus-changed` events the kernel would emit
    // after a successful `spatial_focus` call so the entity-focus
    // bridge can mirror the new FQM.
    mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
      const a = (args ?? {}) as Record<string, unknown>;
      if (cmd === "spatial_focus") {
        const fq = a.fq as FullyQualifiedMoniker;
        const handlers = listeners.get("focus-changed") ?? [];
        for (const h of handlers) {
          h({
            payload: {
              window_label: "main",
              prev_fq: null,
              next_fq: fq,
              next_segment: null,
            },
          });
        }
        return undefined;
      }
      return undefined;
    });

    const { getByTestId, unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <EntityFocusProvider>
            <FocusedFqProbe />
            <FauxInspectorLayer firstFieldSegment="field:task:T1.title" />
          </EntityFocusProvider>
        </FocusLayer>
      </SpatialFocusProvider>,
    );

    // Wait long enough for: (a) the inspector FocusLayer's push
    // effect, (b) the queueMicrotask from `useFirstFieldFocusProbe`,
    // (c) the `nav.focus` dispatch's spatial_focus IPC, (d) the
    // synthetic focus-changed emit propagating through the
    // entity-focus bridge.
    await waitFor(() => {
      const focusCalls = spatialFocusCalls();
      expect(focusCalls.length).toBeGreaterThan(0);
    });

    // The first-field FQM dispatched lives under the inspector layer.
    const focusCalls = spatialFocusCalls();
    const inspectorFocusCall = focusCalls.find((fq) =>
      fq.startsWith("/window/inspector/"),
    );
    expect(inspectorFocusCall).toBe("/window/inspector/field:task:T1.title");

    // The entity-focus store mirrors the FQM after the kernel's
    // `focus-changed` event flows through the bridge.
    await waitFor(
      () => {
        expect(getByTestId("focused-probe").textContent).toBe(
          "/window/inspector/field:task:T1.title",
        );
      },
      { timeout: 1000 },
    );

    unmount();
  });
});
