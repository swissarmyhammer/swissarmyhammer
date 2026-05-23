/**
 * Browser test for card `01KR7CDEFWWVF4WH0BCHE8Y21J`'s
 * `nav.jump`-targets-the-topmost-layer guarantee at the *window* layer.
 *
 * No inspector is open, so the topmost layer is the window root. Pressing
 * `s` (the `nav.jump` keybinding) opens `<JumpToOverlay>`; the overlay's
 * `useJumpTargets` enumerates scopes registered in the topmost layer
 * (read via `topLayerFq()`). Result: pills paint over `<FocusScope>`s
 * that live in the window layer.
 *
 * The companion test
 * `jump-to-overlay.over-inspector.browser.test.tsx` covers the
 * inspector-on-top case.
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
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { asSegment, type FullyQualifiedMoniker } from "@/types/spatial";
import { mkRect, stubScopeGeometry } from "@/test/stub-scope-geometry";

// ---------------------------------------------------------------------------
// Stubbed jump-code generator.
// ---------------------------------------------------------------------------

function installInvokeStub(jumpCodes: string[]) {
  const focusCalls: FullyQualifiedMoniker[] = [];
  mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
    const a = (args ?? {}) as Record<string, unknown>;
    if (cmd === "spatial_focus") {
      focusCalls.push(a.fq as FullyQualifiedMoniker);
      return undefined;
    }
    if (cmd === "generate_jump_codes") {
      const count = (a.count as number) ?? 0;
      return jumpCodes.slice(0, count);
    }
    return undefined;
  });
  return { focusCalls };
}

/**
 * Defer the overlay open by one tick so the seed scopes' mount-time
 * registration with the layer-scope-registry has flushed before the
 * overlay enumerates.
 */
function DeferredJumpToOverlay({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const [actuallyOpen, setActuallyOpen] = React.useState(false);
  React.useEffect(() => {
    if (!open) {
      setActuallyOpen(false);
      return;
    }
    const id = setTimeout(() => setActuallyOpen(true), 0);
    return () => clearTimeout(id);
  }, [open]);
  return <JumpToOverlay open={actuallyOpen} onClose={onClose} />;
}

async function flush(ms = 10) {
  await act(async () => {
    await new Promise((r) => setTimeout(r, ms));
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("<JumpToOverlay> — window layer (no inspector open)", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
  });

  it("enumerates the window-layer's scopes when the window is the topmost layer", async () => {
    installInvokeStub(["a", "s", "d"]);
    const rects = new Map<string, DOMRect>([
      ["seed-card-0", mkRect(10, 10, 100, 30)],
      ["seed-card-1", mkRect(120, 10, 100, 30)],
      ["seed-card-2", mkRect(230, 10, 100, 30)],
    ]);
    const cleanup = stubScopeGeometry(rects);

    const onClose = vi.fn();
    const { unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <EntityFocusProvider>
            {/* Three seed scopes registered under the window layer.
                The Jump-To overlay must enumerate exactly these. */}
            <FocusScope moniker={asSegment("card:0")} data-testid="seed-card-0">
              <span>card 0</span>
            </FocusScope>
            <FocusScope moniker={asSegment("card:1")} data-testid="seed-card-1">
              <span>card 1</span>
            </FocusScope>
            <FocusScope moniker={asSegment("card:2")} data-testid="seed-card-2">
              <span>card 2</span>
            </FocusScope>
            <DeferredJumpToOverlay open={true} onClose={onClose} />
          </EntityFocusProvider>
        </FocusLayer>
      </SpatialFocusProvider>,
    );

    await flush(50);

    // Each enumerated scope renders a `data-jump-fq`-tagged pill. The
    // overlay's enumeration runs against `topLayerFq()`, which here is
    // the window layer — so the only enumerated FQMs are the
    // window-layer scopes, prefixed with `/window/`. The pills render
    // into a portal at document-body, so we query off `document`
    // rather than the rendering container.
    const pills = Array.from(
      document.querySelectorAll<HTMLElement>("[data-jump-code]"),
    );
    expect(pills.length).toBe(3);
    const fqs = pills.map((p) => p.dataset.jumpFq).sort();
    for (const fq of fqs) {
      expect(fq).toMatch(/^\/window\//);
    }
    expect(fqs).toEqual(["/window/card:0", "/window/card:1", "/window/card:2"]);

    // Three unique jump codes assigned (one per scope).
    const codes = pills.map((p) => p.dataset.jumpCode);
    expect(new Set(codes).size).toBe(3);

    cleanup();
    unmount();
  });
});
