/**
 * Regression test for cards 01KQ7GM77B1E6YH8Z893K05VKY (the original
 * boolean-prop fix) and 01KQ7K7KZNR3EHS9SY0XY79NYE (this card —
 * replacing the boolean prop with the `<Inspectable>` wrapper).
 *
 * Background: double-clicking a perspective tab used to open the inspector
 * because every `<FocusScope>` / `<FocusZone>` unconditionally dispatched
 * `ui.inspect` on double-click. Perspectives are not entities, so that was
 * an architectural mismatch. The first fix made the dispatch opt-in via
 * an `inspectOnDoubleClick` boolean prop on the primitives; the second
 * fix replaced that prop with a dedicated `<Inspectable>` wrapper
 * (`inspectable.tsx`). The perspective-bar wrapper and each
 * `perspective_tab:<id>` leaf are NOT wrapped in `<Inspectable>`, so a
 * double-click on them dispatches nothing at the wrapping level. The
 * tab button's own `onDoubleClick` still calls `startRename`, so inline
 * rename mode stays intact.
 *
 * This file exercises the post-fix contract end-to-end. The assertions
 * make no reference to the underlying mechanism — they pin the
 * user-visible behavior — so they continue to pass after the wrapper
 * refactor:
 *
 *   1. **Regression**: dblclick on a tab does NOT dispatch `ui.inspect`
 *      against `perspective_tab:*`, but the inline rename editor still
 *      mounts because the tab button's own handler runs.
 *   2. **Bar background**: dblclick on the `ui:perspective-bar` zone
 *      whitespace also does NOT dispatch `ui.inspect`.
 *   3. **Single-click still focuses**: a single click on a tab still
 *      fires `spatial_focus(key)` — we did not regress the focus path.
 *
 * Mock pattern matches `perspective-bar.spatial.test.tsx` /
 * `grid-view.nav-is-eventdriven.test.tsx`: `vi.hoisted` builds the
 * `mockInvoke` / `mockListen` / `listeners` triple shared across every
 * spatial-stack test in the project.
 *
 * Runs under `kanban-app/ui/vite.config.ts`'s browser project (real
 * Chromium via Playwright). Files matching `*.test.tsx` outside
 * `*.node.test.ts` land there.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent, act, waitFor } from "@testing-library/react";
import { TooltipProvider } from "@/components/ui/tooltip";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before component imports.
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
  listen: (...a: Parameters<typeof mockListen>) => mockListen(...a),
  emit: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
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
// Perspective + view + UI mocks — match the shape used by the existing
// perspective-tab-bar tests.
// ---------------------------------------------------------------------------

/** Mutable mock perspective shape — the test toggles which tabs are present. */
type MockPerspective = {
  id: string;
  name: string;
  view: string;
  filter?: string;
  group?: string;
};

let mockPerspectivesValue = {
  perspectives: [] as MockPerspective[],
  activePerspective: null as MockPerspective | null,
  setActivePerspectiveId: vi.fn(),
  refresh: vi.fn(() => Promise.resolve()),
};

vi.mock("@/lib/perspective-context", () => ({
  usePerspectives: () => mockPerspectivesValue,
}));

let mockViewsValue = {
  views: [{ id: "board-1", name: "Board", kind: "board", icon: "kanban" }],
  activeView: { id: "board-1", name: "Board", kind: "board", icon: "kanban" },
  setActiveViewId: vi.fn(),
  refresh: vi.fn(() => Promise.resolve()),
};

vi.mock("@/lib/views-context", () => ({
  useViews: () => mockViewsValue,
}));

vi.mock("@/lib/context-menu", () => ({
  useContextMenu: () => vi.fn(),
}));

vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({ getEntities: () => [] }),
}));

vi.mock("@/components/window-container", () => ({
  useBoardData: () => ({ virtualTagMeta: [] }),
}));

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => ({ entity: { name: "task", fields: [] }, fields: [] }),
    getFieldDef: () => undefined,
    mentionableTypes: [],
    loading: false,
  }),
}));

vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => ({
    keymap_mode: "cua",
    scope_chain: [],
    open_boards: [],
    has_clipboard: false,
    clipboard_entity_type: null,
    windows: {},
    recent_boards: [],
  }),
  useUIStateLoading: () => ({
    state: {
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      has_clipboard: false,
      clipboard_entity_type: null,
      windows: {},
      recent_boards: [],
    },
    loading: false,
  }),
}));

// ---------------------------------------------------------------------------
// Imports — after mocks
// ---------------------------------------------------------------------------

import { PerspectiveTabBar } from "./perspective-tab-bar";
import { FocusLayer } from "./focus-layer";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { asLayerName, type SpatialKey } from "@/types/spatial";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Wait for register effects scheduled in `useEffect` to flush. */
async function flushSetup() {
  // Two ticks: first runs the registration `useEffect`, second lets any
  // promise-resolved follow-on (e.g. listener registration) settle.
  await act(async () => {
    await Promise.resolve();
  });
  await act(async () => {
    await Promise.resolve();
  });
}

/** Render the bar wrapped in the production-shaped spatial-nav stack. */
function renderBar(): ReturnType<typeof render> {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asLayerName("window")}>
        <TooltipProvider delayDuration={100}>
          <PerspectiveTabBar />
        </TooltipProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

/** Collect every `dispatch_command` call's args, in order. */
function dispatchCommandCalls(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "dispatch_command")
    .map((c) => c[1] as Record<string, unknown>);
}

/** Collect every `spatial_focus` call's args, in order. */
function spatialFocusCalls(): Array<{ key: SpatialKey }> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_focus")
    .map((c) => c[1] as { key: SpatialKey });
}

/** Collect every `spatial_register_zone` invocation argument bag. */
function registerZoneArgs(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_zone")
    .map((c) => c[1] as Record<string, unknown>);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("PerspectiveTabBar — perspective is NOT an entity (regression)", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    mockPerspectivesValue = {
      perspectives: [
        { id: "p1", name: "Sprint", view: "board" },
        { id: "p2", name: "Backlog", view: "board" },
      ],
      activePerspective: { id: "p1", name: "Sprint", view: "board" },
      setActivePerspectiveId: vi.fn(),
      refresh: vi.fn(() => Promise.resolve()),
    };
    mockViewsValue = {
      views: [{ id: "board-1", name: "Board", kind: "board", icon: "kanban" }],
      activeView: {
        id: "board-1",
        name: "Board",
        kind: "board",
        icon: "kanban",
      },
      setActiveViewId: vi.fn(),
      refresh: vi.fn(() => Promise.resolve()),
    };
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // -------------------------------------------------------------------------
  // Test 1 — Regression: dblclick on perspective tab does NOT inspect
  // -------------------------------------------------------------------------

  it("dblclick on a perspective tab does NOT dispatch ui.inspect", async () => {
    const { container, queryByRole, unmount } = renderBar();
    await flushSetup();

    const tabNode = container.querySelector(
      "[data-moniker='perspective_tab:p1']",
    ) as HTMLElement | null;
    expect(tabNode).not.toBeNull();

    // Reset invoke before the gesture so we measure only the dblclick's
    // IPC and any rename dispatches it would normally trigger.
    mockInvoke.mockClear();

    fireEvent.doubleClick(tabNode!);

    // The button's own onDoubleClick fires `startRename`, which mounts
    // an InlineRenameEditor in place of the tab text. That confirms the
    // legacy rename path is preserved.
    await waitFor(() => {
      // The inline rename editor mounts a CM6 textbox as the only
      // input role inside the tab button.
      expect(queryByRole("textbox")).not.toBeNull();
    });

    // No ui.inspect dispatch — neither via dispatch_command nor against
    // any `perspective_tab:*` target.
    const inspectCalls = dispatchCommandCalls().filter(
      (c) => c.cmd === "ui.inspect",
    );
    expect(inspectCalls).toHaveLength(0);

    // Defensive: nothing whose first IPC arg matches /inspect/i fired
    // either — covers any future reshuffle of the IPC name.
    const anyInspect = mockInvoke.mock.calls.find((c) => {
      const cmd = typeof c[0] === "string" ? c[0] : "";
      const payload = (c[1] as { cmd?: string } | undefined)?.cmd ?? "";
      return /inspect/i.test(cmd) || /inspect/i.test(payload);
    });
    expect(anyInspect).toBeUndefined();

    unmount();
  });

  // -------------------------------------------------------------------------
  // Test 2 — Regression: dblclick on the bar background does NOT inspect
  // -------------------------------------------------------------------------

  it("dblclick on the ui:perspective-bar zone background does NOT dispatch ui.inspect", async () => {
    const { container, unmount } = renderBar();
    await flushSetup();

    const barNode = container.querySelector(
      "[data-moniker='ui:perspective-bar']",
    ) as HTMLElement | null;
    expect(barNode).not.toBeNull();

    mockInvoke.mockClear();

    // Double-click the bar wrapper itself, not a tab inside it.
    fireEvent.doubleClick(barNode!);

    const inspectCalls = dispatchCommandCalls().filter(
      (c) => c.cmd === "ui.inspect",
    );
    expect(inspectCalls).toHaveLength(0);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Test 3 — Single-click still focuses (regression guard for focus path)
  // -------------------------------------------------------------------------

  it("single-click on a tab still dispatches spatial_focus for THAT tab's key", async () => {
    const { container, unmount } = renderBar();
    await flushSetup();

    // Capture the tab's spatial key from its registration call so we
    // can assert against the precise key, not just any focus dispatch.
    const tabRegistrations = mockInvoke.mock.calls
      .filter((c) => c[0] === "spatial_register_scope")
      .map((c) => c[1] as Record<string, unknown>);
    const p1Tab = tabRegistrations.find(
      (r) => r.moniker === "perspective_tab:p1",
    );
    expect(p1Tab).toBeTruthy();

    const tabNode = container.querySelector(
      "[data-moniker='perspective_tab:p1']",
    ) as HTMLElement | null;
    expect(tabNode).not.toBeNull();

    mockInvoke.mockClear();

    fireEvent.click(tabNode!);

    const focusCalls = spatialFocusCalls();
    expect(focusCalls).toHaveLength(1);
    expect(focusCalls[0].key).toBe(p1Tab!.key);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Sanity: the bar zone is registered (so the dblclick target above
  // resolves to a real DOM node, not a stale selector).
  // -------------------------------------------------------------------------

  it("registers the ui:perspective-bar zone on mount (sanity)", async () => {
    const { unmount } = renderBar();
    await flushSetup();

    const barZone = registerZoneArgs().find(
      (a) => a.moniker === "ui:perspective-bar",
    );
    expect(barZone).toBeTruthy();

    unmount();
  });
});
