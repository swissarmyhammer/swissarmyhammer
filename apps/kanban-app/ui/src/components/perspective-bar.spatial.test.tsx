/**
 * Browser-mode test for the perspective bar's spatial-nav behaviour.
 *
 * Source of truth for acceptance of card `01KPZS32YN7CRNM0TH7GR28M86`. The
 * bar wraps its row in a `<FocusScope moniker="ui:perspective-bar">` and each
 * tab in a `<FocusScope moniker="perspective_tab:{id}">` leaf. This file
 * exercises the click → `spatial_focus` → `focus-changed` → React state →
 * `<FocusIndicator>` chain end-to-end so a regression in any link surfaces
 * here.
 *
 * Mock pattern matches `grid-view.nav-is-eventdriven.test.tsx`:
 *   - `vi.hoisted` builds an invoke / listen mock pair the test owns.
 *   - `mockListen` records every `listen("focus-changed", cb)` callback so
 *     `fireFocusChanged(key)` can drive the React tree as if the Rust
 *     kernel had emitted a `focus-changed` event.
 *
 * Runs under `kanban-app/ui/vite.config.ts`'s browser project (real Chromium
 * via Playwright) — every `*.test.tsx` outside `*.node.test.tsx` lands here.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent, act, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
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
// perspective-tab-bar tests so the bar mounts without surprise.
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
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import {
  asSegment,
  type FocusChangedPayload,
  type FullyQualifiedMoniker,
  type WindowLabel,
} from "@/types/spatial";

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
 * Drive a `focus-changed` event into the React tree as if the Rust kernel
 * had emitted one for the current window.
 *
 * The provider's listener decides which side of the swap fires — we always
 * pass both `prev_fq` and `next_fq` to mimic the kernel's payload shape.
 * Wrapping the dispatch in `act()` flushes the React state updates so the
 * caller can assert against post-update DOM in the next tick.
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

/** Render the bar wrapped in the production-shaped spatial-nav stack. */
function renderBar(): ReturnType<typeof render> {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <TooltipProvider delayDuration={100}>
            <PerspectiveTabBar />
          </TooltipProvider>
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

/** Helper: wrap arbitrary children in the same provider stack as `renderBar`. */
function withSpatialStack(children: ReactNode) {
  return (
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <TooltipProvider delayDuration={100}>{children}</TooltipProvider>
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>
  );
}

/** Collect every `spatial_register_scope` invocation argument bag. */
function registerScopeArgs(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_scope")
    .map((c) => c[1] as Record<string, unknown>);
}

/** Collect every `spatial_focus` call's args, in order. */
function spatialFocusCalls(): Array<{ fq: FullyQualifiedMoniker }> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_focus")
    .map((c) => c[1] as { fq: FullyQualifiedMoniker });
}

/** Collect every `spatial_unregister_scope` call's args, in order. */
function unregisterScopeCalls(): Array<{ fq: FullyQualifiedMoniker }> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_unregister_scope")
    .map((c) => c[1] as { fq: FullyQualifiedMoniker });
}

/** True when the moniker matches one of the two accepted bar-zone monikers. */
function isBarMoniker(m: unknown): boolean {
  return m === "ui:perspective-bar" || m === "ui:perspective.bar";
}

/** True when the moniker matches `^perspective_tab:.+$`. */
function isTabMoniker(m: unknown): boolean {
  return typeof m === "string" && /^perspective_tab:.+$/.test(m);
}

/** True when the moniker matches `^filter_editor:.+$`. */
function isFilterEditorMoniker(m: unknown): boolean {
  return typeof m === "string" && /^filter_editor:.+$/.test(m);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("PerspectiveTabBar — browser spatial behaviour", () => {
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

  it("registers a ui:perspective-bar zone (test #1)", async () => {
    const { unmount } = renderBar();
    await flushSetup();

    const barZone = registerScopeArgs().find((a) => isBarMoniker(a.segment));
    expect(barZone).toBeTruthy();
    expect(typeof barZone!.fq).toBe("string");
    expect(barZone!.layerFq).toBeTruthy();
    expect(barZone!.parentZone).toBeNull();

    unmount();
  });

  it("registers a perspective_tab:{id} zone per visible tab (test #2)", async () => {
    const { unmount } = renderBar();
    await flushSetup();

    // After the iteration-2 reshape (card 01KQQSVS4EBKKFN5SS7MW5P8CN) the
    // tab wrapper is a `<FocusScope>` with `perspective_tab.name`,
    // `perspective_tab.filter`, and `perspective_tab.group` leaves
    // inside it. Mirror entity-card iteration 2.
    const tabZones = registerScopeArgs().filter((a) => isTabMoniker(a.segment));
    const monikers = tabZones.map((a) => a.segment as string).sort();
    expect(monikers).toEqual(["perspective_tab:p1", "perspective_tab:p2"]);

    // Each tab zone's parentZone is the bar zone's key — the inner
    // name / filter / group leaves are siblings inside each tab zone,
    // and the tab zones themselves are siblings inside the bar.
    const barZone = registerScopeArgs().find((a) => isBarMoniker(a.segment))!;
    for (const tab of tabZones) {
      expect(tab.parentZone).toBe(barZone.fq);
      expect(tab.layerFq).toBe(barZone.layerFq);
    }

    unmount();
  });

  it("clicking a tab dispatches exactly one spatial_focus for the tab zone (test #3)", async () => {
    const { container, unmount } = renderBar();
    await flushSetup();

    // Post-`8789dcc15`, the inner `perspective_tab.name:{id}` FocusScope
    // was dropped — the outer `perspective_tab:{id}` is itself the
    // focusable target. Clicking anywhere on the tab dispatches
    // `spatial_focus(perspective_tab:{id})` from the outer wrapper.
    const barZone = registerScopeArgs().find((a) => isBarMoniker(a.segment))!;
    const p1Tab = registerScopeArgs().find(
      (a) => a.segment === "perspective_tab:p1",
    )!;

    // Reset invoke before the click so we measure only the click's IPC.
    mockInvoke.mockClear();

    const tabNode = container.querySelector(
      "[data-segment='perspective_tab:p1']",
    ) as HTMLElement | null;
    expect(tabNode).not.toBeNull();

    fireEvent.click(tabNode!);

    const focusCalls = spatialFocusCalls();
    expect(focusCalls).toHaveLength(1);
    expect(focusCalls[0].fq).toBe(p1Tab.fq);
    // The bar zone key must NOT also receive a focus call — the tab
    // stops propagation so the click does not bubble to the wrapping bar.
    expect(focusCalls.find((c) => c.fq === barZone.fq)).toBeUndefined();

    unmount();
  });

  it("focus claim flips data-focused on the tab wrapper and mounts the FocusIndicator (test #4)", async () => {
    const { container, queryByTestId, unmount } = renderBar();
    await flushSetup();

    // The outer `perspective_tab:{id}` wrapper inherits `<FocusScope>`'s
    // default `showFocus={true}` — focused tabs paint a
    // `<FocusIndicator>` (dashed-border inset) and the `data-focused`
    // attribute flips for e2e selectors / debugging tooling.
    const p1Tab = registerScopeArgs().find(
      (a) => a.segment === "perspective_tab:p1",
    )!;

    // No indicator before the focus claim.
    expect(queryByTestId("focus-indicator")).toBeNull();

    await fireFocusChanged({ next_fq: p1Tab.fq as FullyQualifiedMoniker });

    const tabNode = container.querySelector(
      "[data-segment='perspective_tab:p1']",
    ) as HTMLElement;
    await waitFor(() => {
      expect(tabNode.getAttribute("data-focused")).not.toBeNull();
    });

    // The `<FocusIndicator>` mounts inside the focused tab wrapper —
    // the wrapper's default `showFocus={true}` paints the dashed-border
    // indicator on focus.
    const indicator = queryByTestId("focus-indicator");
    expect(indicator).not.toBeNull();
    expect(tabNode.contains(indicator!)).toBe(true);

    unmount();
  });

  it("focus claim on the bar zone flips data-focused but renders no indicator (test #5)", async () => {
    // Container zones use `showFocus={false}` so the visible bar around
    // the entire row would be visual noise. The data-focused attribute
    // still flips so e2e selectors and debugging tooling can observe the
    // claim, but no `<FocusIndicator>` mounts.
    const { container, queryByTestId, unmount } = renderBar();
    await flushSetup();

    const barZone = registerScopeArgs().find((a) => isBarMoniker(a.segment))!;
    const barNode = container.querySelector(
      `[data-segment='${barZone.segment as string}']`,
    ) as HTMLElement;
    expect(barNode).not.toBeNull();
    expect(barNode.getAttribute("data-focused")).toBeNull();

    await fireFocusChanged({ next_fq: barZone.fq as FullyQualifiedMoniker });

    await waitFor(() => {
      expect(barNode.getAttribute("data-focused")).not.toBeNull();
    });
    expect(queryByTestId("focus-indicator")).toBeNull();

    unmount();
  });

  // ---------------------------------------------------------------------
  // Tests #6 (keystrokes → navigate) and #7 (Enter → drill-in) are exercised
  // by the global keymap pipeline rather than by the bar itself: the bar
  // must NOT attach a `keydown` DOM listener (enforced by
  // `perspective-spatial-nav.guards.node.test.ts`). ArrowLeft / ArrowRight
  // and Enter are bound at `<AppShell>` to the `nav.left` / `nav.right` /
  // `nav.drillIn` commands, which dispatch `spatial_navigate` /
  // `spatial_drill_in` for the currently-focused [`FullyQualifiedMoniker`]. The
  // app-shell side of that contract is covered in `app-shell.test.tsx`
  // (`nav.drillIn invokes spatial_drill_in for the focused FullyQualifiedMoniker on
  // Enter`); the bar side of the contract is "do nothing", which the
  // source-level guards already enforce.
  // ---------------------------------------------------------------------

  it("each tab unregisters via spatial_unregister_scope on unmount (test #8)", async () => {
    const { unmount } = renderBar();
    await flushSetup();

    // After the reshape `perspective_tab:{id}` is a zone — `<FocusScope>`
    // unregistration also flows through `spatial_unregister_scope` (the
    // shared kernel sink), so the test's invariant — every registered
    // tab key gets a corresponding unregister call — still holds.
    const tabZones = registerScopeArgs().filter((a) => isTabMoniker(a.segment));
    expect(tabZones.length).toBeGreaterThanOrEqual(2);
    const tabKeys = tabZones.map((a) => a.fq as FullyQualifiedMoniker);

    mockInvoke.mockClear();
    unmount();

    const unregisterKeys = unregisterScopeCalls().map((c) => c.fq);
    for (const k of tabKeys) {
      expect(unregisterKeys).toContain(k);
    }
  });

  it("emits no legacy entity_focus_* / claim_when_* / broadcast_nav_* IPCs (test #9)", async () => {
    const { container, unmount } = renderBar();
    await flushSetup();

    const tabNode = container.querySelector(
      "[data-segment='perspective_tab:p1']",
    ) as HTMLElement | null;
    expect(tabNode).not.toBeNull();
    fireEvent.click(tabNode!);

    const banned = /^(entity_focus_|claim_when_|broadcast_nav_)/;
    const offenders = mockInvoke.mock.calls
      .map((c) => c[0])
      .filter((cmd) => typeof cmd === "string" && banned.test(cmd));
    expect(offenders).toEqual([]);

    unmount();
  });

  it("registers a filter_editor:{activePerspectiveId} scope as a peer of the perspective tabs", async () => {
    // The filter formula bar must register exactly one FocusScope leaf with
    // segment `filter_editor:${activePerspectiveId}` — distinct per
    // perspective, parented to the `ui:perspective-bar` zone, on the same
    // layer as the tabs. This is what makes the formula bar a navigable
    // peer of the tabs in the spatial graph.
    const { unmount } = renderBar();
    await flushSetup();

    const activeId = mockPerspectivesValue.activePerspective!.id;
    const filterScopes = registerScopeArgs().filter((a) =>
      isFilterEditorMoniker(a.segment),
    );
    expect(filterScopes).toHaveLength(1);
    expect(filterScopes[0].segment).toBe(`filter_editor:${activeId}`);

    const barZone = registerScopeArgs().find((a) => isBarMoniker(a.segment))!;
    expect(filterScopes[0].parentZone).toBe(barZone.fq);
    expect(filterScopes[0].layerFq).toBe(barZone.layerFq);

    unmount();
  });

  it("driving focus-changed to the filter_editor leaf flips data-focused on the formula bar", async () => {
    // After registration, dispatching a `focus-changed` event whose
    // `next_fq` matches the filter editor leaf's FQM must flip
    // `data-focused="true"` on its DOM node and mount a `<FocusIndicator>`
    // inside it.
    const { container, queryByTestId, unmount } = renderBar();
    await flushSetup();

    const activeId = mockPerspectivesValue.activePerspective!.id;
    const filterLeaf = registerScopeArgs().find(
      (a) => a.segment === `filter_editor:${activeId}`,
    )!;
    expect(filterLeaf).toBeTruthy();

    await fireFocusChanged({
      next_fq: filterLeaf.fq as FullyQualifiedMoniker,
    });

    await waitFor(() => {
      const node = container.querySelector(
        `[data-segment='filter_editor:${activeId}'][data-focused]`,
      );
      expect(node).not.toBeNull();
    });

    const focusedNode = container.querySelector(
      `[data-segment='filter_editor:${activeId}']`,
    ) as HTMLElement;
    const indicator = queryByTestId("focus-indicator")!;
    expect(indicator).not.toBeNull();
    expect(focusedNode.contains(indicator)).toBe(true);

    unmount();
  });

  it("switching perspectives unregisters the previous filter_editor leaf and registers the next", async () => {
    // The `key={activePerspective.id}` remount on `<FilterFormulaBar>` must
    // drive the kernel through a clean unregister → register cycle when the
    // active perspective changes, so the moniker `filter_editor:${id}` always
    // tracks the currently active perspective rather than aliasing across
    // them.
    const { rerender, unmount } = renderBar();
    await flushSetup();

    const prevId = mockPerspectivesValue.activePerspective!.id;
    const prevLeaf = registerScopeArgs().find(
      (a) => a.segment === `filter_editor:${prevId}`,
    )!;
    expect(prevLeaf).toBeTruthy();

    // Reset invoke mock so we measure only the IPC produced by the switch.
    mockInvoke.mockClear();

    // Flip the active perspective to p2 and rerender — the mocked
    // `usePerspectives` reads from `mockPerspectivesValue` on every render.
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      activePerspective: { id: "p2", name: "Backlog", view: "board" },
    };
    rerender(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <EntityFocusProvider>
            <TooltipProvider delayDuration={100}>
              <PerspectiveTabBar />
            </TooltipProvider>
          </EntityFocusProvider>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const unregisterKeys = unregisterScopeCalls().map((c) => c.fq);
    expect(unregisterKeys).toContain(prevLeaf.fq);

    const nextLeaf = registerScopeArgs().find(
      (a) => a.segment === `filter_editor:p2`,
    );
    expect(nextLeaf).toBeTruthy();
    expect(nextLeaf!.segment).toBe("filter_editor:p2");

    unmount();
  });

  it("focus follows the kernel's claim across perspective tabs (indicator follows data-focused)", async () => {
    // Belt-and-suspenders for #4: when focus moves between perspective
    // tab wrappers, `data-focused="true"` and the `<FocusIndicator>`
    // must follow exactly one wrapper at a time. The wrapper inherits
    // `showFocus={true}` so the dashed-border indicator paints on
    // whichever tab currently holds spatial focus.
    const { container, queryAllByTestId, unmount } = withSpatialStackRendered();
    await flushSetup();

    const p1Tab = registerScopeArgs().find(
      (a) => a.segment === "perspective_tab:p1",
    )!;
    const p2Tab = registerScopeArgs().find(
      (a) => a.segment === "perspective_tab:p2",
    )!;

    await fireFocusChanged({ next_fq: p1Tab.fq as FullyQualifiedMoniker });
    await waitFor(() => {
      const p1 = container.querySelector(
        "[data-segment='perspective_tab:p1']",
      ) as HTMLElement;
      expect(p1.getAttribute("data-focused")).not.toBeNull();
    });
    // The indicator mounts inside p1 — exactly one indicator on the
    // bar at a time.
    {
      const p1Node = container.querySelector(
        "[data-segment='perspective_tab:p1']",
      ) as HTMLElement;
      const indicators = queryAllByTestId("focus-indicator");
      expect(indicators.length).toBe(1);
      expect(p1Node.contains(indicators[0])).toBe(true);
    }

    // Move the claim from p1 → p2. The previously focused wrapper
    // loses its data-focused attribute and its indicator; the next gains both.
    await fireFocusChanged({
      prev_fq: p1Tab.fq as FullyQualifiedMoniker,
      next_fq: p2Tab.fq as FullyQualifiedMoniker,
    });
    await waitFor(() => {
      const p1 = container.querySelector(
        "[data-segment='perspective_tab:p1']",
      ) as HTMLElement;
      const p2 = container.querySelector(
        "[data-segment='perspective_tab:p2']",
      ) as HTMLElement;
      expect(p1.getAttribute("data-focused")).toBeNull();
      expect(p2.getAttribute("data-focused")).not.toBeNull();
    });
    {
      const p2Node = container.querySelector(
        "[data-segment='perspective_tab:p2']",
      ) as HTMLElement;
      const indicators = queryAllByTestId("focus-indicator");
      expect(indicators.length).toBe(1);
      expect(p2Node.contains(indicators[0])).toBe(true);
    }

    unmount();
  });
});

/**
 * Render `<PerspectiveTabBar>` inside the spatial-nav stack.
 *
 * Local helper so the multi-step belt-and-suspenders test does not duplicate
 * the wrapper-render boilerplate. Co-located with the test file rather than
 * exported because the wrapper depends on file-scoped mocks that are not
 * stable across files.
 */
function withSpatialStackRendered() {
  return render(withSpatialStack(<PerspectiveTabBar />));
}
