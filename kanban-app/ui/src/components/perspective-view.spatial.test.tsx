/**
 * Browser-mode test for `<PerspectiveContainer>` + `<ViewContainer>` zone
 * behaviour.
 *
 * Source of truth for acceptance of card `01KPZS32YN7CRNM0TH7GR28M86` and
 * its follow-up that deleted the redundant `ui:view` chrome zone. The
 * perspective container is a viewport-sized chrome zone — it registers in
 * the spatial graph (so the navigator can drill into it) but intentionally
 * does NOT render a visible focus bar around the entire viewport.
 *
 * The `<ViewContainer>` no longer mounts its own `ui:view` zone — it
 * overlapped the inner view's `ui:board` / `ui:grid` zone for the same
 * rect, so it was deleted as a redundant graph hop. The inner view zone is
 * therefore the direct child of `ui:perspective` in the spatial graph.
 *
 * This file pins:
 *
 *   1. The perspective zone registers via `spatial_register_scope` with the
 *      `ui:perspective` moniker and unregisters on unmount.
 *   2. The inner view zone (`ui:board` here, since the active view is
 *      `BoardView`) registers with `parentZone === ui:perspective.fq`.
 *   3. A focus claim on the perspective zone flips `data-focused` for e2e
 *      selectors but does NOT mount `<FocusIndicator>` (because
 *      `showFocusBar={false}` — see the inline comment on the zone).
 *   4. Regression: no `ui:view` zone is ever registered, and no DOM node
 *      with `data-segment='ui:view'` is rendered.
 *
 * Mock pattern matches `grid-view.nav-is-eventdriven.test.tsx`:
 * `vi.hoisted` builds an invoke / listen mock pair; `mockListen` records
 * every `listen("focus-changed", cb)` callback so `fireFocusChanged(key)`
 * can drive the React tree as if the Rust kernel had emitted the event.
 *
 * Runs under the browser project (real Chromium via Playwright) — every
 * `*.test.tsx` outside `*.node.test.tsx` lands there per `vite.config.ts`.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act, waitFor } from "@testing-library/react";
import type { ViewDef, BoardData, PerspectiveDef } from "@/types/kanban";

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
// Container dependencies — the view + perspective containers read from
// several upstream contexts. The test stubs them so the containers mount
// in isolation; the assertions are about the spatial-nav wrapping, not the
// view bodies.
// ---------------------------------------------------------------------------

const mockUsePerspectives = vi.hoisted(() =>
  vi.fn(() => ({
    perspectives: [] as PerspectiveDef[],
    activePerspective: null as PerspectiveDef | null,
    setActivePerspectiveId: vi.fn(),
    refresh: vi.fn(),
  })),
);

vi.mock("@/lib/perspective-context", () => ({
  usePerspectives: () => mockUsePerspectives(),
}));

vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => ({ windows: {} }),
  UIStateProvider: ({ children }: { children: unknown }) => children,
}));

vi.mock("@/components/rust-engine-container", () => ({
  useRefreshEntities: () => () => Promise.resolve({ entities: {} }),
  useEntitiesByType: () => ({}),
}));

vi.mock("@/lib/command-scope", async () => {
  const actual = await vi.importActual<typeof import("@/lib/command-scope")>(
    "@/lib/command-scope",
  );
  return {
    ...actual,
    useActiveBoardPath: () => undefined,
  };
});

const mockViews = vi.hoisted(() =>
  vi.fn(() => ({
    views: [] as ViewDef[],
    activeView: null as ViewDef | null,
    setActiveViewId: vi.fn(),
    refresh: vi.fn(),
  })),
);

vi.mock("@/lib/views-context", () => ({
  ViewsProvider: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
  useViews: () => mockViews(),
}));

const mockBoardData = vi.hoisted(() =>
  vi.fn<() => BoardData | null>(() => null),
);

vi.mock("@/components/window-container", () => ({
  useBoardData: () => mockBoardData(),
}));

// Stub the view bodies so we don't need to mount their data dependencies.
vi.mock("@/components/grouped-board-view", () => ({
  GroupedBoardView: () => <div data-testid="board-view">BoardView</div>,
}));

vi.mock("@/components/grid-view", () => ({
  GridView: () => <div data-testid="grid-view">GridView</div>,
}));

// ---------------------------------------------------------------------------
// Imports — after mocks
// ---------------------------------------------------------------------------

import { PerspectiveContainer } from "./perspective-container";
import { ViewContainer } from "./view-container";
import { FocusLayer } from "./focus-layer";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import {
  asSegment,
  type FocusChangedPayload,
  type FullyQualifiedMoniker,
  type WindowLabel
} from "@/types/spatial";

// ---------------------------------------------------------------------------
// Test data
// ---------------------------------------------------------------------------

const BOARD_VIEW: ViewDef = {
  id: "board-default",
  name: "Board",
  kind: "board",
  icon: "kanban",
};

const MOCK_BOARD: BoardData = {
  board: {
    entity_type: "board",
    id: "b1",
    moniker: "board:b1",
    fields: { name: { String: "Test Board" } },
  },
  columns: [],
  tags: [],
  virtualTagMeta: [],
  summary: {
    total_tasks: 0,
    total_actors: 0,
    ready_tasks: 0,
    blocked_tasks: 0,
    done_tasks: 0,
    percent_complete: 0,
  },
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Wait for register effects scheduled in `useEffect` to flush. */
async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
}

/**
 * Drive a `focus-changed` event into the React tree, mimicking the Rust
 * kernel emitting one for the active window. See the bar test for the full
 * rationale; same shape, same `act()` wrapping so React state updates flush
 * before the next assertion.
 */
async function fireFocusChanged({
  prev_fq = null,
  next_fq = null,
}: {
  prev_fq?: FullyQualifiedMoniker | null;
  next_fq?: FullyQualifiedMoniker | null;
}) {
  const payload: FocusChangedPayload = {
    window_label: "main" as WindowLabel,
    prev_fq,
    next_fq,
    next_segment: null,
  };
  const handlers = listeners.get("focus-changed") ?? [];
  await act(async () => {
    for (const handler of handlers) handler({ payload });
    await Promise.resolve();
  });
}

/**
 * Render the perspective + view containers wrapped in the production-shaped
 * spatial-nav stack. Both containers are mounted because the user's drill-out
 * scenario needs the perspective zone present too — we want to verify the
 * view zone publishes its key into `FocusScopeContext` so the perspective zone
 * can be the chain's parent.
 */
function renderViewStack() {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <PerspectiveContainer>
            <ViewContainer />
          </PerspectiveContainer>
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

/** Collect every `spatial_register_scope` invocation argument bag. */
function registerScopeArgs(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_scope")
    .map((c) => c[1] as Record<string, unknown>);
}

/** Collect every `spatial_unregister_scope` call's args, in order. */
function unregisterScopeCalls(): Array<{ fq: FullyQualifiedMoniker }> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_unregister_scope")
    .map((c) => c[1] as { fq: FullyQualifiedMoniker });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("PerspectiveView (ViewContainer + PerspectiveContainer) — browser spatial behaviour", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    mockViews.mockReturnValue({
      views: [BOARD_VIEW],
      activeView: BOARD_VIEW,
      setActiveViewId: vi.fn(),
      refresh: vi.fn(),
    });
    mockBoardData.mockReturnValue(MOCK_BOARD);
    mockUsePerspectives.mockReturnValue({
      perspectives: [],
      activePerspective: null,
      setActivePerspectiveId: vi.fn(),
      refresh: vi.fn(),
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("does NOT register a ui:view zone on mount — the redundant chrome wrapper was deleted (test #1)", async () => {
    const { container, unmount } = renderViewStack();
    await flushSetup();

    // Regression: no `<FocusScope moniker={asSegment("ui:view")}>` is
    // mounted by `view-container.tsx` anymore. Its rect overlapped the
    // inner view's own viewport-sized zone (`ui:board` / `ui:grid`); the
    // wrapper added no semantic value and was deleted.
    const viewZone = registerScopeArgs().find((a) => a.segment === "ui:view");
    expect(viewZone).toBeUndefined();
    expect(container.querySelector("[data-segment='ui:view']")).toBeNull();

    // The surrounding `ui:perspective` zone is still registered — it owns
    // the chrome rect now and is the spatial-graph parent for whichever
    // inner view zone (`ui:board` / `ui:grid`) the active view body
    // mounts. (View bodies are mocked out in this file; their parent-zone
    // assertion lives in `board-view.spatial-nav.test.tsx` and the
    // end-to-end test.)
    const perspectiveZone = registerScopeArgs().find(
      (a) => a.segment === "ui:perspective",
    );
    expect(perspectiveZone).toBeTruthy();
    expect(typeof perspectiveZone!.fq).toBe("string");
    expect(perspectiveZone!.layerFq).toBeTruthy();

    unmount();
  });

  it("focus claim on the perspective zone flips data-focused but renders no indicator (test #2)", async () => {
    // The perspective zone is viewport-sized chrome — a focus bar around
    // the entire body would be visual noise, so `showFocusBar={false}` is
    // applied at the zone (`perspective-container.tsx`). The
    // `data-focused` attribute must still flip so e2e tooling and the
    // umbrella card (`01KQ5PEHWT...`) verification protocol can observe
    // the claim. (Previously this test targeted the now-deleted `ui:view`
    // chrome zone; the contract moves up one level to its surviving
    // parent.)
    const { container, queryByTestId, unmount } = renderViewStack();
    await flushSetup();

    const perspectiveZone = registerScopeArgs().find(
      (a) => a.segment === "ui:perspective",
    )!;
    const node = container.querySelector(
      "[data-segment='ui:perspective']",
    ) as HTMLElement;
    expect(node).not.toBeNull();
    expect(node.getAttribute("data-focused")).toBeNull();

    await fireFocusChanged({
      next_fq: perspectiveZone.fq as FullyQualifiedMoniker,
    });

    await waitFor(() => {
      expect(node.getAttribute("data-focused")).not.toBeNull();
    });
    // Inline-comment rationale: viewport-sized chrome zones suppress the
    // visible bar; only sized leaves and entities show one. See
    // `perspective-container.tsx` for the production-side comment.
    expect(queryByTestId("focus-indicator")).toBeNull();

    unmount();
  });

  it("drill-out from an inner zone lands on the perspective (test #3)", async () => {
    // Drill-out semantics: when the user is focused on an inner element
    // and Escape pops them out, focus eventually lands on the enclosing
    // chrome zone. With the redundant `ui:view` hop removed, that
    // enclosing zone is `ui:perspective` directly (the inner view zone's
    // direct parent). The bar test mirrors the kernel's emit by
    // dispatching that payload directly — drill-out routing itself lives
    // in the spatial-focus-context tests; what we verify here is that
    // when the kernel does route to the perspective, the React tree
    // follows.
    const { container, unmount } = renderViewStack();
    await flushSetup();

    const perspectiveZone = registerScopeArgs().find(
      (a) => a.segment === "ui:perspective",
    )!;
    const node = container.querySelector(
      "[data-segment='ui:perspective']",
    ) as HTMLElement;

    // Pretend an inner board/grid leaf was focused first; we use a unique
    // key that the registry never minted so it doesn't accidentally match
    // any registered listener.
    const phantomInnerKey = "ffffffff-ffff-4fff-8fff-ffffffffffff" as FullyQualifiedMoniker;
    await fireFocusChanged({ next_fq: phantomInnerKey });
    expect(node.getAttribute("data-focused")).toBeNull();

    // Escape drives a drill-out chain that ultimately pushes focus to
    // the perspective zone. Mimic the kernel's resulting `focus-changed`
    // payload.
    await fireFocusChanged({
      prev_fq: phantomInnerKey,
      next_fq: perspectiveZone.fq as FullyQualifiedMoniker,
    });

    await waitFor(() => {
      expect(node.getAttribute("data-focused")).not.toBeNull();
    });

    unmount();
  });

  it("the perspective zone unregisters via spatial_unregister_scope on unmount (test #4)", async () => {
    const { unmount } = renderViewStack();
    await flushSetup();

    const perspectiveZone = registerScopeArgs().find(
      (a) => a.segment === "ui:perspective",
    )!;
    const expectedKey = perspectiveZone.fq as FullyQualifiedMoniker;

    mockInvoke.mockClear();
    unmount();

    const unregisterKeys = unregisterScopeCalls().map((c) => c.fq);
    expect(unregisterKeys).toContain(expectedKey);
  });

  it("the perspective zone also flips data-focused without an indicator", async () => {
    // Sister contract to test #2 — the surrounding `ui:perspective` zone
    // is also viewport-sized chrome and uses `showFocusBar={false}`.
    // Pinning both halves keeps a regression that turns ONLY the view
    // zone's bar back on (and not the perspective's) from sneaking
    // through under the umbrella card's "any zone with showFocusBar=false
    // has an inline comment" rule.
    const { container, queryByTestId, unmount } = renderViewStack();
    await flushSetup();

    const perspectiveZone = registerScopeArgs().find(
      (a) => a.segment === "ui:perspective",
    )!;
    const node = container.querySelector(
      "[data-segment='ui:perspective']",
    ) as HTMLElement;
    expect(node.getAttribute("data-focused")).toBeNull();

    await fireFocusChanged({ next_fq: perspectiveZone.fq as FullyQualifiedMoniker });

    await waitFor(() => {
      expect(node.getAttribute("data-focused")).not.toBeNull();
    });
    expect(queryByTestId("focus-indicator")).toBeNull();

    unmount();
  });
});
