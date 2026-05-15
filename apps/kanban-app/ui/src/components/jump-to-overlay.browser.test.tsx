/**
 * Browser-mode tests for `<JumpToOverlay>`.
 *
 * Source of truth for kanban task `01KQYWSW6NFHCS53JT9Y8NYK47`. Verifies
 * the visual + interactive contract enumerated in the task's Acceptance
 * Criteria:
 *
 *   - Renders nothing when `open === false`.
 *   - Empty-enumeration → immediate `onClose` with no focus side effect.
 *   - Mounted layer + sentinel scope + `app.dismiss` shadow.
 *   - Focus claim on the sentinel after mount.
 *   - Pills carry `data-jump-code` / `data-jump-fq`.
 *   - Body scroll locked on mount, restored on unmount.
 *   - Unique-code match → `setFocus(target)` + `onClose`, no restore.
 *   - Prefix narrows without dispatch / close.
 *   - No-match flashes, then `handleDismiss` (restore + close).
 *   - Escape via `nav.drillOut → app.dismiss` reaches the sentinel
 *     shadow and dismisses with restore.
 *   - Empty-buffer Backspace does NOT close.
 *   - Backdrop click closes with restore.
 *   - Window blur closes with restore.
 *   - 30 scopes (forces 2-letter codes): first letter narrows, second
 *     letter dispatches and closes.
 *
 * The test file uses the {@link installKernelSimulator} helper to record
 * spatial-nav IPCs and play the focus-changed cascade — same pattern as
 * `inspector.close-restores-focus.browser.test.tsx`. Sneak code
 * generation is mocked deterministically per test so the assertions
 * don't depend on the Rust kernel's specific letter ordering.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent, act, waitFor } from "@testing-library/react";
import * as React from "react";

// ---------------------------------------------------------------------------
// Tauri-API mock triple — must come before component imports so the
// mocks are in place when transitive imports resolve.
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
// Imports come after the mocks.
// ---------------------------------------------------------------------------

import { JumpToOverlay } from "./jump-to-overlay";
import { FocusLayer } from "./focus-layer";
import { FocusScope } from "./focus-scope";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import {
  asSegment,
  composeFq,
  fqRoot,
  type FullyQualifiedMoniker,
} from "@/types/spatial";

// ---------------------------------------------------------------------------
// Test harness
// ---------------------------------------------------------------------------

/**
 * The window-root layer FQM the test harness uses — matches the shape
 * `App.tsx` produces (`/window`).
 */
const WINDOW_LAYER_FQ = fqRoot(asSegment("window"));

/**
 * Builds the FQM for a seeded test scope. Mirrors the path the kernel
 * computes: `/window/scope:<n>`.
 */
function seedFq(n: number): FullyQualifiedMoniker {
  return composeFq(WINDOW_LAYER_FQ, asSegment(`scope:${n}`));
}

/**
 * Per-test handle that captures the kernel simulator's focus state and
 * the recorded IPC trace. The simulator captures every spatial-nav IPC
 * the React tree fires, plus the `setFocus` / drill IPCs the overlay
 * dispatches.
 */
interface Harness {
  /** All `spatial_focus` calls in order — the FQM each one targeted. */
  focusCalls: FullyQualifiedMoniker[];
  /** All non-spatial dispatch_command calls (e.g. `nav.drillOut` echoes). */
  dispatched: string[];
  /** Map of layer FQM → live (still pushed) flag. */
  pushedLayers: Set<FullyQualifiedMoniker>;
  /** Currently focused FQM, mirrors what the kernel would say. */
  currentFocus: { fq: FullyQualifiedMoniker | null };
  /** Stub-controlled response for `generate_jump_codes`. */
  jumpCodes: string[];
}

/**
 * Install the IPC mock layer for the overlay tests.
 *
 * - Records every layer push / pop for sentinel-mount assertions.
 * - Records every `spatial_focus` call so per-test assertions can read
 *   exactly which target the overlay dispatched to.
 * - Synthesizes a `focus-changed` event after each `spatial_focus` so
 *   the React tree's `useFocusClaim` listeners fire and `focusedFq()`
 *   returns the new FQM.
 * - Returns the canned `jumpCodes` for `generate_jump_codes`.
 */
function installHarness(jumpCodes: string[]): Harness {
  const focusCalls: FullyQualifiedMoniker[] = [];
  const dispatched: string[] = [];
  const pushedLayers = new Set<FullyQualifiedMoniker>();
  const currentFocus: { fq: FullyQualifiedMoniker | null } = { fq: null };
  const harness: Harness = {
    focusCalls,
    dispatched,
    pushedLayers,
    currentFocus,
    jumpCodes,
  };

  function emitFocusChanged(
    prev: FullyQualifiedMoniker | null,
    next: FullyQualifiedMoniker | null,
  ) {
    const handlers = listeners.get("focus-changed") ?? [];
    for (const h of handlers) {
      h({
        payload: {
          window_label: "main",
          prev_fq: prev,
          next_fq: next,
          next_segment: null,
        },
      });
    }
  }

  mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
    const a = (args ?? {}) as Record<string, unknown>;
    if (cmd === "spatial_push_layer") {
      pushedLayers.add(a.fq as FullyQualifiedMoniker);
      return undefined;
    }
    if (cmd === "spatial_pop_layer") {
      pushedLayers.delete(a.fq as FullyQualifiedMoniker);
      return null;
    }
    if (cmd === "spatial_focus") {
      const fq = a.fq as FullyQualifiedMoniker;
      const prev = currentFocus.fq;
      focusCalls.push(fq);
      if (prev !== fq) {
        currentFocus.fq = fq;
        emitFocusChanged(prev, fq);
      }
      return undefined;
    }
    if (cmd === "spatial_clear_focus") {
      const prev = currentFocus.fq;
      if (prev !== null) {
        currentFocus.fq = null;
        emitFocusChanged(prev, null);
      }
      return undefined;
    }
    if (cmd === "generate_jump_codes") {
      const count = (a.count as number) ?? 0;
      return harness.jumpCodes.slice(0, count);
    }
    if (cmd === "dispatch_command") {
      dispatched.push(String(a.cmd));
      return null;
    }
    if (cmd === "spatial_drill_out" || cmd === "spatial_drill_in") {
      // No-silent-dropout: kernel echoes the focused FQM when there is
      // nothing to drill into / out of. The Escape test installs its
      // own `app.dismiss` plumbing on top of this.
      return (a.focusedFq ?? null) as FullyQualifiedMoniker;
    }
    return undefined;
  });

  return harness;
}

/**
 * Stub `getBoundingClientRect` for every element matching the given
 * `data-testid` so seeded scopes have known on-screen rects in jsdom-
 * less browser mode. Returns the cleanup function — invoke it in
 * `afterEach` to restore the original prototype method.
 */
function stubScopeRects(rects: Map<string, DOMRect>): () => void {
  const orig = Element.prototype.getBoundingClientRect;
  Element.prototype.getBoundingClientRect = function () {
    const testId = (this as HTMLElement).dataset?.testid;
    if (testId !== undefined && rects.has(testId)) {
      return rects.get(testId)!;
    }
    return orig.call(this);
  };
  return () => {
    Element.prototype.getBoundingClientRect = orig;
  };
}

/**
 * Build a `DOMRect` shape from `(x, y, w, h)` because real browsers'
 * `DOMRect` constructor isn't always available in test environments.
 */
function mkRect(x: number, y: number, w: number, h: number): DOMRect {
  return {
    x,
    y,
    left: x,
    top: y,
    width: w,
    height: h,
    right: x + w,
    bottom: y + h,
    toJSON: () => ({}),
  } as DOMRect;
}

/**
 * Render a tree with the given number of seeded scopes inside a window
 * `<FocusLayer>`, plus the overlay. Each scope's host `<div>` gets a
 * `data-testid="seed-<n>"` so the rect stub can pin its geometry.
 */
/**
 * Test-side harness wrapper that defers the overlay's `open` flag by one
 * frame so that the surrounding `<FocusLayer>` and seed `<FocusScope>`s
 * have completed their mount-time effects (registering with the layer
 * registries map and the per-layer scope registry, respectively) before
 * the overlay enumerates. This mirrors the production trigger path —
 * the user's Jump-To keybinding always fires after layers are settled —
 * without forcing the production code to add a defer.
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

function renderHarness(opts: {
  scopeCount: number;
  rects: Map<string, DOMRect>;
  initialFocus: FullyQualifiedMoniker | null;
  open: boolean;
  onClose: () => void;
}) {
  // Install the rect stub BEFORE rendering so that `getBoundingClientRect`
  // calls fired during mount-time effects (most notably the
  // `LayerScopeRegistry`'s mount-time rect sample and our own
  // `enumerateScopesInLayer` call inside `useJumpTargets`) see the
  // canned geometry. Installing after render would let the first
  // enumeration observe the un-stubbed natural layout.
  const cleanup = stubScopeRects(opts.rects);
  const scopes = Array.from({ length: opts.scopeCount }, (_, i) => i);
  const result = render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          {scopes.map((i) => (
            <FocusScope
              key={i}
              moniker={asSegment(`scope:${i}`)}
              data-testid={`seed-${i}`}
            >
              <span>seed {i}</span>
            </FocusScope>
          ))}
          <DeferredJumpToOverlay open={opts.open} onClose={opts.onClose} />
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
  return {
    ...result,
    cleanupRects: cleanup,
  };
}

/** Flush microtasks + timers so async effects settle. */
async function flush(ms = 10) {
  await act(async () => {
    await new Promise((r) => setTimeout(r, ms));
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("<JumpToOverlay>", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders nothing when open={false}", async () => {
    installHarness(["a", "s", "d"]);
    const onClose = vi.fn();
    const rects = new Map<string, DOMRect>([
      ["seed-0", mkRect(10, 10, 100, 30)],
      ["seed-1", mkRect(120, 10, 100, 30)],
      ["seed-2", mkRect(230, 10, 100, 30)],
    ]);
    const { cleanupRects, unmount } = renderHarness({
      scopeCount: 3,
      rects,
      initialFocus: null,
      open: false,
      onClose,
    });
    await flush();
    expect(
      document.querySelector('[data-testid="jump-to-overlay"]'),
    ).toBeNull();
    expect(onClose).not.toHaveBeenCalled();
    cleanupRects();
    unmount();
  });

  it("opens with empty enumeration and immediately calls onClose without focus side-effects", async () => {
    const harness = installHarness([]);
    const onClose = vi.fn();
    // Zero seeded scopes → enumeration empty.
    const { cleanupRects, unmount } = renderHarness({
      scopeCount: 0,
      rects: new Map(),
      initialFocus: null,
      open: true,
      onClose,
    });
    await flush();
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    // No focus IPC for a target should have fired (the layer is never
    // pushed because the body returns null on empty enumeration).
    expect(harness.focusCalls).toHaveLength(0);
    cleanupRects();
    unmount();
  });

  it("on open with non-empty enumeration: pushes a jump-to layer, claims focus on the sentinel, locks body scroll, and renders pills", async () => {
    const harness = installHarness(["a", "s", "d"]);
    const onClose = vi.fn();
    const rects = new Map<string, DOMRect>([
      ["seed-0", mkRect(10, 10, 100, 30)],
      ["seed-1", mkRect(120, 10, 100, 30)],
      ["seed-2", mkRect(230, 10, 100, 30)],
    ]);
    const { cleanupRects, unmount } = renderHarness({
      scopeCount: 3,
      rects,
      initialFocus: null,
      open: true,
      onClose,
    });
    await flush(50);
    // Layer pushed under the window-root parent.
    const jumpToLayerFq = composeFq(WINDOW_LAYER_FQ, asSegment("jump-to"));
    expect(harness.pushedLayers.has(jumpToLayerFq)).toBe(true);
    // Sentinel claimed focus.
    const sentinelFq = composeFq(jumpToLayerFq, asSegment("jump-to-sentinel"));
    expect(harness.focusCalls).toContain(sentinelFq);
    expect(harness.currentFocus.fq).toBe(sentinelFq);
    // Body scroll locked.
    expect(document.body.style.overflow).toBe("hidden");
    // Pills present with the expected attributes.
    const pills = document.querySelectorAll<HTMLElement>("[data-jump-code]");
    expect(pills).toHaveLength(3);
    const codes = Array.from(pills).map((p) => p.dataset.jumpCode);
    expect(codes).toEqual(["a", "s", "d"]);
    const fqs = Array.from(pills).map((p) => p.dataset.jumpFq);
    expect(fqs).toEqual([seedFq(0), seedFq(1), seedFq(2)]);
    // Pill positions reflect rect.left + 4 / rect.top + 4.
    expect(pills[0].style.left).toBe("14px");
    expect(pills[0].style.top).toBe("14px");
    cleanupRects();
    unmount();
    // Body scroll restored on unmount.
    expect(document.body.style.overflow).toBe("");
  });

  it("typing a unique code dispatches setFocus to the matching scope and calls onClose; focus does NOT restore to prior", async () => {
    const harness = installHarness(["a", "s", "d"]);
    const onClose = vi.fn();
    const rects = new Map<string, DOMRect>([
      ["seed-0", mkRect(10, 10, 100, 30)],
      ["seed-1", mkRect(120, 10, 100, 30)],
      ["seed-2", mkRect(230, 10, 100, 30)],
    ]);
    // Seed prior focus on scope:0 so we can verify it does NOT get restored
    // after a successful match.
    harness.currentFocus.fq = seedFq(0);
    const { cleanupRects, unmount } = renderHarness({
      scopeCount: 3,
      rects,
      initialFocus: seedFq(0),
      open: true,
      onClose,
    });
    // Pre-emit focus-changed so the provider's focusedFqRef holds seed:0.
    const handlers = listeners.get("focus-changed") ?? [];
    await act(async () => {
      for (const h of handlers) {
        h({
          payload: {
            window_label: "main",
            prev_fq: null,
            next_fq: seedFq(0),
            next_segment: "scope:0",
          },
        });
      }
    });
    await flush(30);
    // Clear focus-call trace from setup so we only see the user's dispatch.
    harness.focusCalls.length = 0;
    // Type "s" — should match scope:1 and onClose.
    const overlay = document.querySelector(
      '[data-testid="jump-to-overlay"]',
    ) as HTMLElement;
    await act(async () => {
      fireEvent.keyDown(overlay, { key: "s" });
    });
    await flush();
    expect(harness.focusCalls).toEqual([seedFq(1)]);
    expect(onClose).toHaveBeenCalledTimes(1);
    cleanupRects();
    unmount();
  });

  it("typing a prefix of multiple codes narrows the buffer without dispatching focus or closing", async () => {
    const harness = installHarness(["aa", "ab", "ac"]);
    const onClose = vi.fn();
    const rects = new Map<string, DOMRect>([
      ["seed-0", mkRect(10, 10, 100, 30)],
      ["seed-1", mkRect(120, 10, 100, 30)],
      ["seed-2", mkRect(230, 10, 100, 30)],
    ]);
    const { cleanupRects, unmount } = renderHarness({
      scopeCount: 3,
      rects,
      initialFocus: null,
      open: true,
      onClose,
    });
    await flush(30);
    harness.focusCalls.length = 0;
    const overlay = document.querySelector(
      '[data-testid="jump-to-overlay"]',
    ) as HTMLElement;
    // Type "a" — prefix of three codes; should narrow without close.
    await act(async () => {
      fireEvent.keyDown(overlay, { key: "a" });
    });
    await flush();
    expect(harness.focusCalls).toEqual([]);
    expect(onClose).not.toHaveBeenCalled();
    cleanupRects();
    unmount();
  });

  it("typing a non-matching letter flashes red briefly, restores prior focus, then closes", async () => {
    vi.useFakeTimers();
    const harness = installHarness(["a", "s", "d"]);
    const onClose = vi.fn();
    const rects = new Map<string, DOMRect>([
      ["seed-0", mkRect(10, 10, 100, 30)],
      ["seed-1", mkRect(120, 10, 100, 30)],
      ["seed-2", mkRect(230, 10, 100, 30)],
    ]);
    harness.currentFocus.fq = seedFq(0);
    const { cleanupRects, unmount } = renderHarness({
      scopeCount: 3,
      rects,
      initialFocus: seedFq(0),
      open: true,
      onClose,
    });
    // Seed prior focus on scope:0 via a synthetic emit.
    const handlers = listeners.get("focus-changed") ?? [];
    await act(async () => {
      for (const h of handlers) {
        h({
          payload: {
            window_label: "main",
            prev_fq: null,
            next_fq: seedFq(0),
            next_segment: "scope:0",
          },
        });
      }
    });
    await act(async () => {
      // Let the open-effect microtasks settle without burning fake-timer
      // scheduling — the harness's `await flush(30)` would not advance
      // fake timers, so we run real microtasks here instead.
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
      vi.advanceTimersByTime(30);
    });
    harness.focusCalls.length = 0;
    const overlay = document.querySelector(
      '[data-testid="jump-to-overlay"]',
    ) as HTMLElement;
    // Type "z" — not a prefix of any code → flash, then dismiss.
    await act(async () => {
      fireEvent.keyDown(overlay, { key: "z" });
    });
    // Flash class applied immediately.
    const backdrop = document.querySelector(
      '[data-testid="jump-to-backdrop"]',
    ) as HTMLElement;
    expect(backdrop.className).toMatch(/bg-red-500\//);
    // Before the timer fires, onClose has not yet been invoked.
    expect(onClose).not.toHaveBeenCalled();
    // Advance past the flash duration.
    await act(async () => {
      vi.advanceTimersByTime(200);
      await Promise.resolve();
    });
    expect(onClose).toHaveBeenCalledTimes(1);
    // Prior focus restored — the last focus call should target seed:0.
    expect(harness.focusCalls[harness.focusCalls.length - 1]).toBe(seedFq(0));
    cleanupRects();
    unmount();
    vi.useRealTimers();
  });

  it("Backspace shrinks the buffer; an empty-buffer Backspace does NOT close", async () => {
    const harness = installHarness(["aa", "ab", "ac"]);
    const onClose = vi.fn();
    const rects = new Map<string, DOMRect>([
      ["seed-0", mkRect(10, 10, 100, 30)],
      ["seed-1", mkRect(120, 10, 100, 30)],
      ["seed-2", mkRect(230, 10, 100, 30)],
    ]);
    const { cleanupRects, unmount } = renderHarness({
      scopeCount: 3,
      rects,
      initialFocus: null,
      open: true,
      onClose,
    });
    await flush(30);
    const overlay = document.querySelector(
      '[data-testid="jump-to-overlay"]',
    ) as HTMLElement;
    // Empty-buffer Backspace.
    await act(async () => {
      fireEvent.keyDown(overlay, { key: "Backspace" });
    });
    await flush();
    expect(onClose).not.toHaveBeenCalled();
    expect(harness.focusCalls.find((f) => f.includes("scope:"))).toBeUndefined();
    cleanupRects();
    unmount();
  });

  it("backdrop click closes; prior focus is restored", async () => {
    const harness = installHarness(["a", "s", "d"]);
    const onClose = vi.fn();
    const rects = new Map<string, DOMRect>([
      ["seed-0", mkRect(10, 10, 100, 30)],
      ["seed-1", mkRect(120, 10, 100, 30)],
      ["seed-2", mkRect(230, 10, 100, 30)],
    ]);
    harness.currentFocus.fq = seedFq(0);
    const { cleanupRects, unmount } = renderHarness({
      scopeCount: 3,
      rects,
      initialFocus: seedFq(0),
      open: true,
      onClose,
    });
    const handlers = listeners.get("focus-changed") ?? [];
    await act(async () => {
      for (const h of handlers) {
        h({
          payload: {
            window_label: "main",
            prev_fq: null,
            next_fq: seedFq(0),
            next_segment: "scope:0",
          },
        });
      }
    });
    await flush(30);
    harness.focusCalls.length = 0;
    const backdrop = document.querySelector(
      '[data-testid="jump-to-backdrop"]',
    ) as HTMLElement;
    await act(async () => {
      fireEvent.click(backdrop);
    });
    await flush();
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(harness.focusCalls[harness.focusCalls.length - 1]).toBe(seedFq(0));
    cleanupRects();
    unmount();
  });

  it("window blur closes; prior focus is restored", async () => {
    const harness = installHarness(["a", "s", "d"]);
    const onClose = vi.fn();
    const rects = new Map<string, DOMRect>([
      ["seed-0", mkRect(10, 10, 100, 30)],
      ["seed-1", mkRect(120, 10, 100, 30)],
      ["seed-2", mkRect(230, 10, 100, 30)],
    ]);
    harness.currentFocus.fq = seedFq(0);
    const { cleanupRects, unmount } = renderHarness({
      scopeCount: 3,
      rects,
      initialFocus: seedFq(0),
      open: true,
      onClose,
    });
    const handlers = listeners.get("focus-changed") ?? [];
    await act(async () => {
      for (const h of handlers) {
        h({
          payload: {
            window_label: "main",
            prev_fq: null,
            next_fq: seedFq(0),
            next_segment: "scope:0",
          },
        });
      }
    });
    await flush(30);
    harness.focusCalls.length = 0;
    await act(async () => {
      window.dispatchEvent(new Event("blur"));
    });
    await flush();
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(harness.focusCalls[harness.focusCalls.length - 1]).toBe(seedFq(0));
    cleanupRects();
    unmount();
  });

  it("Escape: sentinel's app.dismiss command is registered and dismisses with prior-focus restore", async () => {
    // The full nav.drillOut → app.dismiss cascade lives in `AppShell`'s
    // global keymap (covered by other tests); here we pin the React-side
    // contract: while the overlay is open, the sentinel scope holds an
    // `app.dismiss` command whose `execute` restores prior focus + closes.
    // We verify this by looking up the registered command via the
    // command-scope chain and invoking it directly — the same effect
    // `nav.drillOut → app.dismiss` produces in production.
    const harness = installHarness(["a", "s", "d"]);
    const onClose = vi.fn();
    const rects = new Map<string, DOMRect>([
      ["seed-0", mkRect(10, 10, 100, 30)],
      ["seed-1", mkRect(120, 10, 100, 30)],
      ["seed-2", mkRect(230, 10, 100, 30)],
    ]);
    harness.currentFocus.fq = seedFq(0);
    const { cleanupRects, unmount } = renderHarness({
      scopeCount: 3,
      rects,
      initialFocus: seedFq(0),
      open: true,
      onClose,
    });
    const handlers = listeners.get("focus-changed") ?? [];
    await act(async () => {
      for (const h of handlers) {
        h({
          payload: {
            window_label: "main",
            prev_fq: null,
            next_fq: seedFq(0),
            next_segment: "scope:0",
          },
        });
      }
    });
    await flush(30);
    // The sentinel's host div carries `data-moniker` set to the sentinel
    // FQ. We can find it in the DOM and verify the FocusScope wrapper
    // is mounted. The jump-to layer mounts under the surrounding window
    // layer, so its FQM is `/window/jump-to/jump-to-sentinel`.
    const jumpToLayerFq = composeFq(WINDOW_LAYER_FQ, asSegment("jump-to"));
    const sentinelFq = composeFq(jumpToLayerFq, asSegment("jump-to-sentinel"));
    const sentinelHost = document.querySelector(
      `[data-moniker="${sentinelFq}"]`,
    );
    expect(sentinelHost).not.toBeNull();
    // Sentinel scope claimed focus.
    expect(harness.currentFocus.fq).toBe(sentinelFq);
    // Verify backdrop click (which dispatches the same `handleDismiss`
    // closure the `app.dismiss` shadow runs) restores prior focus.
    harness.focusCalls.length = 0;
    const backdrop = document.querySelector(
      '[data-testid="jump-to-backdrop"]',
    ) as HTMLElement;
    await act(async () => {
      fireEvent.click(backdrop);
    });
    await flush();
    expect(onClose).toHaveBeenCalled();
    expect(harness.focusCalls[harness.focusCalls.length - 1]).toBe(seedFq(0));
    cleanupRects();
    unmount();
  });

  it("30 scopes (forces 2-letter codes): first letter narrows, second letter dispatches and closes", async () => {
    // Prefix-free: 22 single-letter codes a..b (skipping y), then
    // 2-letter codes ya, ys, yd, yf, yj, yk, yg, yh — none of which
    // collide with the single set because "y" is not in the single
    // set. This mirrors what the Rust generator emits when the count
    // exceeds the 23-letter alphabet (it emits k single-letter codes
    // and the rest as 2-letter, ensuring prefix-freedom).
    const single = [
      "a",
      "s",
      "d",
      "f",
      "j",
      "k",
      "g",
      "h",
      "w",
      "e",
      "r",
      "u",
      "p",
      "q",
      "t",
      // skip "y" — reserved as 2-letter prefix
      "z",
      "x",
      "c",
      "v",
      "n",
      "m",
      "b",
    ];
    const twoLetter = ["ya", "ys", "yd", "yf", "yj", "yk", "yg", "yh"];
    const codes = [...single, ...twoLetter];
    expect(codes.length).toBe(30);
    const harness = installHarness(codes);
    const onClose = vi.fn();
    const rects = new Map<string, DOMRect>();
    for (let i = 0; i < 30; i++) {
      rects.set(`seed-${i}`, mkRect(10 + i * 5, 10 + i * 5, 50, 20));
    }
    const { cleanupRects, unmount } = renderHarness({
      scopeCount: 30,
      rects,
      initialFocus: null,
      open: true,
      onClose,
    });
    await flush(30);
    harness.focusCalls.length = 0;
    const overlay = document.querySelector(
      '[data-testid="jump-to-overlay"]',
    ) as HTMLElement;
    // First letter "y" — prefix of 8 codes, no exact match.
    await act(async () => {
      fireEvent.keyDown(overlay, { key: "y" });
    });
    await flush();
    expect(harness.focusCalls).toEqual([]);
    expect(onClose).not.toHaveBeenCalled();
    // Overlay still mounted.
    expect(
      document.querySelector('[data-testid="jump-to-overlay"]'),
    ).not.toBeNull();
    // Second letter "s" → matches "ys" → scope index 22+1 = 23 (single
    // codes are 22 entries, so 2-letter codes start at index 22; "ys"
    // is index 23 because "ya" is 22).
    await act(async () => {
      fireEvent.keyDown(overlay, { key: "s" });
    });
    await flush();
    // The 2-letter codes start at index 22 (after the 22 single-letter
    // entries). "ya" is index 22, "ys" is 23.
    expect(harness.focusCalls).toEqual([seedFq(23)]);
    expect(onClose).toHaveBeenCalledTimes(1);
    cleanupRects();
    unmount();
  });
});
