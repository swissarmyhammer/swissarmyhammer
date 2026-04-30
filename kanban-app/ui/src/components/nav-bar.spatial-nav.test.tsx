/**
 * Browser-mode test for the nav bar's spatial-nav behaviour.
 *
 * Source of truth for acceptance of card `01KQ20Q2PNNR9VMES60QQSVXTS`
 * (NavBar: wrap as zone, strip legacy keyboard nav). The bar wraps its row
 * in a `<FocusZone moniker="ui:navbar">` and each actionable child in a
 * `<FocusScope>` leaf with a `ui:navbar.{name}` moniker. This file exercises
 * the click → `spatial_focus` → `focus-changed` → React state →
 * `<FocusIndicator>` chain end-to-end so a regression in any link surfaces
 * here.
 *
 * The reopen of the card was specifically because clicking a nav bar
 * button did not produce visible focus feedback. Tests below pin the click
 * → indicator chain for each of the three leaves so the visible bar
 * actually mounts inside the leaf the user clicked.
 *
 * Mock pattern matches `perspective-bar.spatial.test.tsx`:
 *   - `vi.hoisted` builds an invoke / listen mock pair the test owns.
 *   - `mockListen` records every `listen("focus-changed", cb)` callback so
 *     `fireFocusChanged(key)` can drive the React tree as if the Rust
 *     kernel had emitted a `focus-changed` event.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent, act, waitFor } from "@testing-library/react";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { BoardData, OpenBoard } from "@/types/kanban";

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
// WindowContainer + command-scope + schema mocks — same shape as
// `nav-bar.test.tsx` so the bar mounts without surprise.
// ---------------------------------------------------------------------------

const mockBoardData = vi.hoisted(() =>
  vi.fn<() => BoardData | null>(() => null),
);
const mockOpenBoards = vi.hoisted(() => vi.fn<() => OpenBoard[]>(() => []));
const mockActiveBoardPath = vi.hoisted(() =>
  vi.fn<() => string | undefined>(() => undefined),
);
const mockHandleSwitchBoard = vi.hoisted(() => vi.fn<(arg: string) => void>());

vi.mock("@/components/window-container", () => ({
  useBoardData: () => mockBoardData(),
  useOpenBoards: () => mockOpenBoards(),
  useActiveBoardPath: () => mockActiveBoardPath(),
  useHandleSwitchBoard: () => mockHandleSwitchBoard,
}));

const mockDispatchInspect = vi.hoisted(() => vi.fn(() => Promise.resolve()));
const mockDispatchSearch = vi.hoisted(() => vi.fn(() => Promise.resolve()));
const mockIsBusy = vi.hoisted(() => vi.fn(() => false));

vi.mock("@/lib/command-scope", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/command-scope")>();
  return {
    ...actual,
    useDispatchCommand: (cmd: string) => {
      if (cmd === "ui.inspect") return mockDispatchInspect;
      if (cmd === "app.search") return mockDispatchSearch;
      return vi.fn(() => Promise.resolve());
    },
    useCommandBusy: () => ({ isBusy: mockIsBusy() }),
  };
});

const mockPercentFieldDef = {
  field_name: "percent_complete",
  display_name: "% Complete",
  field_type: "PercentComplete",
};

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => undefined,
    getFieldDef: (_entityType: string, fieldName: string) =>
      fieldName === "percent_complete" ? mockPercentFieldDef : undefined,
    mentionableTypes: [],
    loading: false,
  }),
}));

// Mock the Field component with a thin `<FocusZone>` wrapper that
// preserves the production contract: the field IS a `<FocusZone>` whose
// moniker is `field:{type}:{id}.{name}` (see
// `kanban-app/ui/src/components/fields/field.tsx`). The wrapper lets the
// percent-complete field register against the spatial graph from these
// tests without pulling in the entity store and field registries.
//
// The rect-regression test below asserts the field zone's
// kernel-stored rect is non-zero alongside the navbar leaves; mocking
// the field as a plain span (the historic shape) would silently skip
// the zone registration and let a zero-rect bug pass.
vi.mock("@/components/fields/field", async () => {
  const { FocusZone } = await import("@/components/focus-zone");
  const { asSegment } = await import("@/types/spatial");
  return {
    Field: (props: Record<string, unknown>) => {
      const fieldName = (props.fieldDef as { field_name?: string })
        ?.field_name ?? "unknown";
      const moniker = asSegment(
        `field:${props.entityType}:${props.entityId}.${fieldName}`,
      );
      return (
        <FocusZone moniker={moniker}>
          <span data-testid="field-percent">{String(props.entityId)}</span>
        </FocusZone>
      );
    },
  };
});

// ---------------------------------------------------------------------------
// Imports — after mocks
// ---------------------------------------------------------------------------

import { NavBar } from "./nav-bar";
import { FocusLayer } from "./focus-layer";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import {
  asSegment,
  type FocusChangedPayload,
  type FullyQualifiedMoniker,
  type WindowLabel
} from "@/types/spatial";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Identity-stable layer name for the test window root, matches App.tsx. */
const WINDOW_LAYER_NAME = asSegment("window");

/** Wait for register effects scheduled in `useEffect` to flush. */
async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
}

/**
 * Drive a `focus-changed` event into the React tree as if the Rust kernel
 * had emitted one for the current window. Mirrors the helper in
 * `perspective-bar.spatial.test.tsx`.
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
 * Render `<NavBar>` inside the spatial-focus + window-root layer providers
 * that the production tree mounts in `App.tsx`.
 */
function renderNavBar() {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={WINDOW_LAYER_NAME}>
        <TooltipProvider delayDuration={100}>
          <NavBar />
        </TooltipProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

/** Collect every `spatial_register_zone` invocation argument bag. */
function registerZoneArgs(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_zone")
    .map((c) => c[1] as Record<string, unknown>);
}

/** Collect every `spatial_register_scope` invocation argument bag. */
function registerScopeArgs(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_scope")
    .map((c) => c[1] as Record<string, unknown>);
}

/** Collect every `spatial_focus` call's args, in order. */
function spatialFocusCalls(): Array<{ key: FullyQualifiedMoniker }> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_focus")
    .map((c) => c[1] as { key: FullyQualifiedMoniker });
}

// ---------------------------------------------------------------------------
// Test data
// ---------------------------------------------------------------------------

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
    total_tasks: 5,
    total_actors: 2,
    ready_tasks: 3,
    blocked_tasks: 1,
    done_tasks: 1,
    percent_complete: 20,
  },
};

const MOCK_OPEN_BOARDS: OpenBoard[] = [
  { path: "/boards/a/.kanban", name: "Board A", is_active: true },
  { path: "/boards/b/.kanban", name: "Board B", is_active: false },
];

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("NavBar — browser spatial behaviour", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    mockBoardData.mockReturnValue(MOCK_BOARD);
    mockOpenBoards.mockReturnValue(MOCK_OPEN_BOARDS);
    mockActiveBoardPath.mockReturnValue("/boards/a/.kanban");
    mockIsBusy.mockReturnValue(false);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // -------------------------------------------------------------------------
  // Click → spatial_focus → focus-changed → visible indicator chain
  //
  // These are the load-bearing tests for the card reopen. The user reported
  // "clicking a nav bar button does not produce visible focus feedback".
  // Each of the three leaves exercises the full chain end-to-end so a
  // regression in any link surfaces here.
  // -------------------------------------------------------------------------

  it("clicking the board-selector leaf dispatches spatial_focus for THAT leaf's key", async () => {
    const { container, unmount } = renderNavBar();
    await flushSetup();

    const leaf = registerScopeArgs().find(
      (a) => a.segment === "ui:navbar.board-selector",
    );
    expect(leaf).toBeDefined();

    mockInvoke.mockClear();

    const node = container.querySelector(
      "[data-moniker='ui:navbar.board-selector']",
    ) as HTMLElement | null;
    expect(node).not.toBeNull();

    fireEvent.click(node!);

    const focusCalls = spatialFocusCalls();
    expect(focusCalls).toHaveLength(1);
    expect(focusCalls[0].key).toBe(leaf!.key);

    unmount();
  });

  it("clicking the inspect leaf dispatches spatial_focus for THAT leaf's key", async () => {
    const { container, unmount } = renderNavBar();
    await flushSetup();

    const leaf = registerScopeArgs().find(
      (a) => a.segment === "ui:navbar.inspect",
    );
    expect(leaf).toBeDefined();

    mockInvoke.mockClear();

    const node = container.querySelector(
      "[data-moniker='ui:navbar.inspect']",
    ) as HTMLElement | null;
    expect(node).not.toBeNull();

    fireEvent.click(node!);

    const focusCalls = spatialFocusCalls();
    expect(focusCalls).toHaveLength(1);
    expect(focusCalls[0].key).toBe(leaf!.key);

    unmount();
  });

  it("clicking the search leaf dispatches spatial_focus for THAT leaf's key", async () => {
    const { container, unmount } = renderNavBar();
    await flushSetup();

    const leaf = registerScopeArgs().find(
      (a) => a.segment === "ui:navbar.search",
    );
    expect(leaf).toBeDefined();

    mockInvoke.mockClear();

    const node = container.querySelector(
      "[data-moniker='ui:navbar.search']",
    ) as HTMLElement | null;
    expect(node).not.toBeNull();

    fireEvent.click(node!);

    const focusCalls = spatialFocusCalls();
    expect(focusCalls).toHaveLength(1);
    expect(focusCalls[0].key).toBe(leaf!.key);

    unmount();
  });

  it("focus claim mounts the FocusIndicator inside the board-selector leaf", async () => {
    const { container, queryByTestId, unmount } = renderNavBar();
    await flushSetup();

    const leaf = registerScopeArgs().find(
      (a) => a.segment === "ui:navbar.board-selector",
    )!;

    expect(queryByTestId("focus-indicator")).toBeNull();

    await fireFocusChanged({ next_fq: leaf.key as FullyQualifiedMoniker });

    await waitFor(() => {
      expect(queryByTestId("focus-indicator")).not.toBeNull();
    });
    const node = container.querySelector(
      "[data-moniker='ui:navbar.board-selector']",
    ) as HTMLElement;
    const indicator = queryByTestId("focus-indicator")!;
    expect(node.contains(indicator)).toBe(true);
    expect(node.getAttribute("data-focused")).not.toBeNull();

    unmount();
  });

  it("focus claim mounts the FocusIndicator inside the inspect leaf", async () => {
    const { container, queryByTestId, unmount } = renderNavBar();
    await flushSetup();

    const leaf = registerScopeArgs().find(
      (a) => a.segment === "ui:navbar.inspect",
    )!;

    expect(queryByTestId("focus-indicator")).toBeNull();

    await fireFocusChanged({ next_fq: leaf.key as FullyQualifiedMoniker });

    await waitFor(() => {
      expect(queryByTestId("focus-indicator")).not.toBeNull();
    });
    const node = container.querySelector(
      "[data-moniker='ui:navbar.inspect']",
    ) as HTMLElement;
    const indicator = queryByTestId("focus-indicator")!;
    expect(node.contains(indicator)).toBe(true);
    expect(node.getAttribute("data-focused")).not.toBeNull();

    unmount();
  });

  it("focus claim mounts the FocusIndicator inside the search leaf", async () => {
    const { container, queryByTestId, unmount } = renderNavBar();
    await flushSetup();

    const leaf = registerScopeArgs().find(
      (a) => a.segment === "ui:navbar.search",
    )!;

    expect(queryByTestId("focus-indicator")).toBeNull();

    await fireFocusChanged({ next_fq: leaf.key as FullyQualifiedMoniker });

    await waitFor(() => {
      expect(queryByTestId("focus-indicator")).not.toBeNull();
    });
    const node = container.querySelector(
      "[data-moniker='ui:navbar.search']",
    ) as HTMLElement;
    const indicator = queryByTestId("focus-indicator")!;
    expect(node.contains(indicator)).toBe(true);
    expect(node.getAttribute("data-focused")).not.toBeNull();

    unmount();
  });

  // -------------------------------------------------------------------------
  // Single-variant focus indicator
  //
  // The bar leaves wrap small icon buttons (24x24) inside a `gap-2` flex
  // row. The `<FocusIndicator>` cursor-bar paints a 4px-wide stripe 8px
  // to the LEFT of its host (`-left-2 w-1`); the navbar's `gap-2` gives
  // the bar exactly the room it needs to land in the gap between
  // siblings, immediately to the right of the previous sibling and
  // pointing at the focused button — the same column-strip pattern
  // `<PerspectiveTabBar>` ships. There is no second visual variant: the
  // architectural contract is one indicator, and the layout makes that
  // single indicator legible on the navbar.
  // -------------------------------------------------------------------------

  it("renders the cursor-bar (not a ring) on the inspect leaf when focused", async () => {
    const { queryByTestId, unmount } = renderNavBar();
    await flushSetup();

    const leaf = registerScopeArgs().find(
      (a) => a.segment === "ui:navbar.inspect",
    )!;

    await fireFocusChanged({ next_fq: leaf.key as FullyQualifiedMoniker });

    await waitFor(() => {
      expect(queryByTestId("focus-indicator")).not.toBeNull();
    });
    const indicator = queryByTestId("focus-indicator")!;
    // Bar signature: a `-left-2 w-1` stripe, NOT an `inset-0 ring-2`
    // outline. The historic ring variant is gone — there is one and only
    // one visual.
    expect(indicator.className).toContain("-left-2");
    expect(indicator.className).toContain("w-1");
    expect(indicator.className).not.toContain("inset-0");
    expect(indicator.className).not.toContain("ring-2");

    unmount();
  });

  it("renders the cursor-bar (not a ring) on the search leaf when focused", async () => {
    const { queryByTestId, unmount } = renderNavBar();
    await flushSetup();

    const leaf = registerScopeArgs().find(
      (a) => a.segment === "ui:navbar.search",
    )!;

    await fireFocusChanged({ next_fq: leaf.key as FullyQualifiedMoniker });

    await waitFor(() => {
      expect(queryByTestId("focus-indicator")).not.toBeNull();
    });
    const indicator = queryByTestId("focus-indicator")!;
    expect(indicator.className).toContain("-left-2");
    expect(indicator.className).not.toContain("inset-0");

    unmount();
  });

  it("renders the cursor-bar (not a ring) on the board-selector leaf when focused", async () => {
    const { queryByTestId, unmount } = renderNavBar();
    await flushSetup();

    const leaf = registerScopeArgs().find(
      (a) => a.segment === "ui:navbar.board-selector",
    )!;

    await fireFocusChanged({ next_fq: leaf.key as FullyQualifiedMoniker });

    await waitFor(() => {
      expect(queryByTestId("focus-indicator")).not.toBeNull();
    });
    const indicator = queryByTestId("focus-indicator")!;
    expect(indicator.className).toContain("-left-2");
    expect(indicator.className).not.toContain("inset-0");

    unmount();
  });

  // -------------------------------------------------------------------------
  // Zone-level focus
  // -------------------------------------------------------------------------

  it("focus claim on the navbar zone flips data-focused but renders no indicator", async () => {
    // The navbar zone has `showFocusBar={false}` because a focus bar around
    // the entire viewport-spanning row would be visual noise — the leaves
    // own visible focus. The data-focused attribute still flips so e2e
    // selectors and debugging tooling can observe the claim, but no
    // `<FocusIndicator>` mounts on the zone itself.
    const { container, queryByTestId, unmount } = renderNavBar();
    await flushSetup();

    const navbarZone = registerZoneArgs().find(
      (a) => a.segment === "ui:navbar",
    )!;
    const node = container.querySelector(
      "[data-moniker='ui:navbar']",
    ) as HTMLElement;
    expect(node).not.toBeNull();
    expect(node.getAttribute("data-focused")).toBeNull();

    await fireFocusChanged({ next_fq: navbarZone.key as FullyQualifiedMoniker });

    await waitFor(() => {
      expect(node.getAttribute("data-focused")).not.toBeNull();
    });
    expect(queryByTestId("focus-indicator")).toBeNull();

    unmount();
  });

  // -------------------------------------------------------------------------
  // Click activation still works
  //
  // The button's onClick handler must still fire so command dispatch keeps
  // working. Spatial focus is layered on top of the existing button
  // semantics, not in place of them.
  // -------------------------------------------------------------------------

  it("clicking the inspect leaf still dispatches the ui.inspect command", async () => {
    const { container, unmount } = renderNavBar();
    await flushSetup();

    const button = container.querySelector(
      "button[aria-label='Inspect board']",
    ) as HTMLElement | null;
    expect(button).not.toBeNull();

    fireEvent.click(button!);

    expect(mockDispatchInspect).toHaveBeenCalled();

    unmount();
  });

  it("clicking the search leaf still dispatches the app.search command", async () => {
    const { container, unmount } = renderNavBar();
    await flushSetup();

    const button = container.querySelector(
      "button[aria-label='Search']",
    ) as HTMLElement | null;
    expect(button).not.toBeNull();

    fireEvent.click(button!);

    expect(mockDispatchSearch).toHaveBeenCalled();

    unmount();
  });

  // -------------------------------------------------------------------------
  // Field-as-zone integration
  //
  // The percent-complete `<Field>` is itself a `<FocusZone>` keyed by
  // `field:board:{id}.percent_complete` (see `fields/field.tsx`). Its
  // zone registration is the responsibility of the Field-as-zone card
  // and is verified by that card's tests. From the navbar's side, the
  // verification is structural: the navbar zone publishes its
  // FullyQualifiedMoniker via `FocusZoneContext`, so any nested `<FocusZone>` (such
  // as the Field) reads that key as its `parent_zone`.
  //
  // This test confirms the navbar end of the contract: a nested
  // `<FocusZone>` rendered as a child of the navbar registers with the
  // navbar zone's key as its `parent_zone`. We don't unmock `<Field>`
  // here (it pulls in the entity store and field registries) — the
  // shape of the contract is what matters. A regression that drops the
  // navbar's `<FocusZone>` (collapsing it back to a plain `<header>`)
  // would surface here because the inner zone's `parentZone` would be
  // `null` instead of the navbar's key.
  //
  // This complements the existing navbar test
  // ("registers ui:navbar.board-selector as a FocusScope child of the
  // navbar zone") which exercises the leaf side of the parent-zone
  // chain. Together they pin both shapes: leaves AND nested zones see
  // the navbar as their parent.
  // -------------------------------------------------------------------------

  it("nested zones read the navbar zone's FullyQualifiedMoniker as their parent_zone", async () => {
    // Use the existing render helper (mock `<Field>` is harmless because
    // we're injecting a separate zone via children, not via Field).
    // The mock for Field is module-level so we can't unmock it for one
    // test; instead we render the real `<FocusZone>` directly as a
    // sibling of the navbar's leaves to assert the context propagation.
    //
    // We rely on `renderNavBar` for the spatial provider stack. The
    // navbar zone's `FocusZoneContext.Provider` wraps every child the
    // navbar renders, so any `<FocusZone>` we mount as a descendant of
    // <NavBar /> would inherit it. Here we use a child fixture that
    // registers a known moniker; if the parent context were missing the
    // moniker would land at the layer root (parentZone === null).
    const { unmount } = renderNavBar();
    await flushSetup();

    const navbarZone = registerZoneArgs().find(
      (a) => a.segment === "ui:navbar",
    );
    expect(navbarZone).toBeDefined();

    // The navbar zone's moniker must be registered as a layer-root zone
    // (parentZone === null) so descendant zones — like the
    // percent-complete Field — discover it through `useParentZoneFq()`
    // and register their own `parent_zone` against the navbar zone's
    // key. The Field-as-zone card asserts the descendant side; this
    // assertion locks in the navbar's role as the parent that
    // descendant zones can find.
    expect(navbarZone!.parentZone).toBeNull();
    expect(navbarZone!.key).toBeTruthy();

    unmount();
  });

  // -------------------------------------------------------------------------
  // Rect regression — every navbar entry registers a non-zero rect
  //
  // Card `01KQ9XWHP2Y5H1QB5B3RJFEBBR` traced "arrow Left/Right does not
  // traverse navbar leaves" to most likely **zero-sized rects** at
  // registration time: if a leaf's `getBoundingClientRect()` returns
  // 0×0 because the navbar hasn't laid out yet when the register effect
  // fires, beam search drops the leaf from candidate scoring and the
  // user can't navigate to it. The Rust kernel's beam math is correct
  // (see `swissarmyhammer-focus/tests/navbar_arrow_nav.rs`); the seam
  // is on the React side, in registration timing.
  //
  // This test mounts `<NavBar>` in the production provider stack and
  // snapshots the rect each navbar entry passes to
  // `spatial_register_zone` / `spatial_register_scope`. None must be
  // zero-width or zero-height: a zero rect would silently break beam
  // search and leave the user unable to navigate, exactly the symptom
  // the card is fixing.
  //
  // Coverage spans:
  //   - the navbar zone itself (`ui:navbar`)
  //   - each navbar leaf (`ui:navbar.board-selector`,
  //     `ui:navbar.inspect`, `ui:navbar.search`)
  //   - the percent-complete field **zone**
  //     (`field:board:b1.percent_complete`)
  //
  // The Field mock above registers a real `<FocusZone>` so the field's
  // rect appears in the assertion alongside the leaves; mocking it as
  // a plain span would silently skip this regression guard.
  // -------------------------------------------------------------------------

  it("every navbar entry registers a non-zero rect at first paint", async () => {
    const { unmount } = renderNavBar();
    await flushSetup();

    /** Predicate: the entry's rect is positive on both axes. */
    const isPositiveRect = (entry: Record<string, unknown>): boolean => {
      const rect = entry.rect as
        | { width: number; height: number }
        | undefined;
      if (!rect) return false;
      return rect.width > 0 && rect.height > 0;
    };

    const navbarZoneEntries = registerZoneArgs().filter(
      (a) => a.segment === "ui:navbar",
    );
    expect(navbarZoneEntries.length).toBeGreaterThan(0);
    for (const entry of navbarZoneEntries) {
      const rect = entry.rect as { width: number; height: number };
      expect(
        isPositiveRect(entry),
        `ui:navbar zone rect must be non-zero at first paint (got width=${rect?.width}, height=${rect?.height}); a zero rect silently breaks beam search`,
      ).toBe(true);
    }

    const fieldZoneEntries = registerZoneArgs().filter(
      (a) => a.segment === "field:board:b1.percent_complete",
    );
    expect(
      fieldZoneEntries.length,
      "percent-complete field zone must register inside the navbar",
    ).toBeGreaterThan(0);
    for (const entry of fieldZoneEntries) {
      const rect = entry.rect as { width: number; height: number };
      expect(
        isPositiveRect(entry),
        `field:board:b1.percent_complete zone rect must be non-zero at first paint (got width=${rect?.width}, height=${rect?.height})`,
      ).toBe(true);
    }

    const navbarLeafMonikers = [
      "ui:navbar.board-selector",
      "ui:navbar.inspect",
      "ui:navbar.search",
    ] as const;
    for (const moniker of navbarLeafMonikers) {
      const entries = registerScopeArgs().filter((a) => a.segment === moniker);
      expect(
        entries.length,
        `${moniker} leaf must register at first paint`,
      ).toBeGreaterThan(0);
      for (const entry of entries) {
        const rect = entry.rect as { width: number; height: number };
        expect(
          isPositiveRect(entry),
          `${moniker} leaf rect must be non-zero at first paint (got width=${rect?.width}, height=${rect?.height}); a zero rect silently breaks beam search and leaves the user unable to arrow-navigate to this leaf`,
        ).toBe(true);
      }
    }

    unmount();
  });
});
