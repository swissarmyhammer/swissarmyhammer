/**
 * Browser test for card `01KR7CDEFWWVF4WH0BCHE8Y21J`'s
 * `nav.jump`-targets-the-topmost-layer guarantee when an inspector
 * layer is open on top of the window root.
 *
 * Setup: window layer → inspector layer (pushed on top) →
 * `<JumpToOverlay open={true}>`. The overlay's `useJumpTargets`
 * enumerates scopes registered in the topmost layer, which is now the
 * inspector layer. Result: pills paint over inspector-layer
 * `<FocusScope>`s, NOT over the window-layer cards beneath.
 *
 * The companion test `jump-to-overlay.window-layer.browser.test.tsx`
 * covers the no-inspector case.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act } from "@testing-library/react";
import * as React from "react";

// ---------------------------------------------------------------------------
// Hoisted Tauri-API spy triple.
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

import { JumpToOverlay } from "./jump-to-overlay";
import { FocusLayer } from "./focus-layer";
import { FocusScope } from "./focus-scope";
import { useEnclosingLayerFq } from "./layer-fq-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { asSegment } from "@/types/spatial";
import { mkRect, stubScopeGeometry } from "@/test/stub-scope-geometry";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function installInvokeStub(jumpCodes: string[]) {
  const pushedLayers: string[] = [];
  const poppedLayers: string[] = [];
  const registeredScopes: Array<{ fq: string; layerFq: string }> = [];
  mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
    const a = (args ?? {}) as Record<string, unknown>;
    if (cmd === "generate_jump_codes") {
      const count = (a.count as number) ?? 0;
      return jumpCodes.slice(0, count);
    }
    if (cmd === "spatial_push_layer") {
      pushedLayers.push(String(a.fq));
      return undefined;
    }
    if (cmd === "spatial_pop_layer") {
      poppedLayers.push(String(a.fq));
      return null;
    }
    if (cmd === "spatial_register_scope") {
      registeredScopes.push({
        fq: String(a.fq),
        layerFq: String(a.layerFq),
      });
      return undefined;
    }
    return undefined;
  });
  return { pushedLayers, poppedLayers, registeredScopes };
}

/**
 * Wraps `<FocusLayer name="inspector">` so the parent layer FQM can be
 * read explicitly — the inspector layer is mounted as a sibling of
 * other content in production (`InspectorsContainer`), so the
 * test mirrors that shape.
 *
 * The inspector layer mounts on the next tick to mirror the production
 * timing: the window layer pushes on its own effect, then the user
 * clicks an `Inspectable` which mounts the inspector layer. Mounting
 * everything in a single render flips the React effect order
 * (child-before-parent), making the inspector push fire before the
 * window push and inverting the conceptual stack.
 */
function DeferredInspectorLayer({ children }: { children: React.ReactNode }) {
  const windowLayerFq = useEnclosingLayerFq();
  const [mounted, setMounted] = React.useState(false);
  React.useEffect(() => {
    const id = setTimeout(() => setMounted(true), 0);
    return () => clearTimeout(id);
  }, []);
  if (!mounted) return null;
  return (
    <FocusLayer name={asSegment("inspector")} parentLayerFq={windowLayerFq}>
      {children}
    </FocusLayer>
  );
}

function DeferredJumpToOverlay({
  open,
  onClose,
  delayMs = 20,
}: {
  open: boolean;
  onClose: () => void;
  delayMs?: number;
}) {
  const [actuallyOpen, setActuallyOpen] = React.useState(false);
  React.useEffect(() => {
    if (!open) {
      setActuallyOpen(false);
      return;
    }
    // Wait long enough that both the window layer's effect and the
    // deferred inspector layer's setTimeout(0)+effect have fired
    // BEFORE the overlay's enumeration runs. The exact value is
    // generous on purpose — the alternative (a tighter race) makes
    // the test flake under varying CI load.
    const id = setTimeout(() => setActuallyOpen(true), delayMs);
    return () => clearTimeout(id);
  }, [open, delayMs]);
  return <JumpToOverlay open={actuallyOpen} onClose={onClose} />;
}

async function flush(ms = 60) {
  await act(async () => {
    await new Promise((r) => setTimeout(r, ms));
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("<JumpToOverlay> — over inspector (inspector layer is topmost)", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
  });

  it("enumerates inspector-layer scopes (NOT window-layer scopes) when an inspector layer is on top", async () => {
    const harness = installInvokeStub(["a", "s", "d"]);
    const rects = new Map<string, DOMRect>([
      ["seed-card-0", mkRect(10, 10, 100, 30)],
      ["seed-card-1", mkRect(120, 10, 100, 30)],
      ["seed-field-0", mkRect(500, 10, 100, 30)],
      ["seed-field-1", mkRect(500, 50, 100, 30)],
      ["seed-field-2", mkRect(500, 90, 100, 30)],
    ]);
    const cleanup = stubScopeGeometry(rects);

    const onClose = vi.fn();
    const { unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <EntityFocusProvider>
            {/* Window-layer cards — must NOT appear in jump enumeration. */}
            <FocusScope moniker={asSegment("card:0")} data-testid="seed-card-0">
              <span>card 0</span>
            </FocusScope>
            <FocusScope moniker={asSegment("card:1")} data-testid="seed-card-1">
              <span>card 1</span>
            </FocusScope>

            {/* Inspector layer pushed on top — mounted on the next
                tick to mirror production timing (the window layer
                pushes first, then user opens an inspector). Three
                inspector field scopes — these ARE what jump-to
                should enumerate. */}
            <DeferredInspectorLayer>
              <FocusScope
                moniker={asSegment("field:task:T1.title")}
                data-testid="seed-field-0"
              >
                <span>title</span>
              </FocusScope>
              <FocusScope
                moniker={asSegment("field:task:T1.status")}
                data-testid="seed-field-1"
              >
                <span>status</span>
              </FocusScope>
              <FocusScope
                moniker={asSegment("field:task:T1.body")}
                data-testid="seed-field-2"
              >
                <span>body</span>
              </FocusScope>
            </DeferredInspectorLayer>

            <DeferredJumpToOverlay open={true} onClose={onClose} />
          </EntityFocusProvider>
        </FocusLayer>
      </SpatialFocusProvider>,
    );

    // Two timers chain: deferred inspector layer (0ms), then
    // deferred overlay (20ms). Wait long enough that both have
    // fired and their effects have completed.
    await flush(80);

    // The overlay enumerates against `topLayerFq()`, which is the
    // inspector layer (most-recently-pushed). Only the three field
    // scopes should produce pills; the two window-layer cards must
    // NOT appear.
    const pills = Array.from(
      document.querySelectorAll<HTMLElement>("[data-jump-code]"),
    );
    const debugFqs = pills.map((p) => p.dataset.jumpFq);
    expect(
      pills.length,
      `pushed: ${JSON.stringify(harness.pushedLayers)}; ` +
        `popped: ${JSON.stringify(harness.poppedLayers)}; ` +
        `scopes: ${JSON.stringify(harness.registeredScopes)}; ` +
        `enumerated FQs: ${JSON.stringify(debugFqs)}`,
    ).toBe(3);

    const fqs = pills.map((p) => p.dataset.jumpFq).sort();
    for (const fq of fqs) {
      // Every enumerated FQM lives under the inspector layer.
      expect(fq).toMatch(/^\/window\/inspector\/field:/);
    }
    expect(fqs).toEqual([
      "/window/inspector/field:task:T1.body",
      "/window/inspector/field:task:T1.status",
      "/window/inspector/field:task:T1.title",
    ]);

    // The window-layer card FQMs must NOT appear in the enumeration.
    expect(fqs).not.toContain("/window/card:0");
    expect(fqs).not.toContain("/window/card:1");

    // Pills paint above z-30 (above inspector panel chrome). Card
    // `01KR7CDEFWWVF4WH0BCHE8Y21J` step 7 added `z-[80]` to the
    // pills and the chrome backdrop so they paint over the
    // inspector. The Tailwind arbitrary-value class compiles to
    // `z-index: 80`. Assert via class presence (the runtime CSS
    // resolution depends on Tailwind preprocessing being available
    // in the test environment, which it is, but checking the source
    // class is the more deterministic shape — the class is the
    // contract).
    for (const pill of pills) {
      expect(pill.className).toContain("z-[80]");
    }

    cleanup();
    unmount();
  });
});
