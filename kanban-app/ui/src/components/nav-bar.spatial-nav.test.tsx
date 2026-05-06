/**
 * Browser-mode test for the nav bar's spatial-nav behaviour.
 *
 * Source of truth for acceptance of card `01KQ20Q2PNNR9VMES60QQSVXTS`
 * (NavBar: wrap as zone, strip legacy keyboard nav). The bar is a plain
 * `<div role="banner">` and each actionable child is a `<FocusScope>` leaf
 * with a `ui:navbar.{name}` moniker, registered as a peer top-level scope
 * under the surrounding window `<FocusLayer>`. This file exercises the
 * click → `spatial_focus` → `focus-changed` → React state →
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

// Mock the Field component with a thin `<FocusScope>` wrapper that
// preserves the production contract: the field IS a `<FocusScope>` whose
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
  const { FocusScope } = await import("@/components/focus-scope");
  const { asSegment } = await import("@/types/spatial");
  return {
    Field: (props: Record<string, unknown>) => {
      const fieldName =
        (props.fieldDef as { field_name?: string })?.field_name ?? "unknown";
      const moniker = asSegment(
        `field:${props.entityType}:${props.entityId}.${fieldName}`,
      );
      return (
        <FocusScope moniker={moniker}>
          <span data-testid="field-percent">{String(props.entityId)}</span>
        </FocusScope>
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
  type WindowLabel,
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

  it("clicking the board-selector zone dispatches spatial_focus for THAT zone's key", async () => {
    // The board-selector is a zone (multi-leaf surface), not a scope —
    // the kernel rejects a scope wrapping further focus primitives via the
    // scope-is-leaf invariant. See
    // swissarmyhammer-focus/tests/scope_is_leaf.rs.
    const { container, unmount } = renderNavBar();
    await flushSetup();

    const zone = registerScopeArgs().find(
      (a) => a.segment === "ui:navbar.board-selector",
    );
    expect(zone).toBeDefined();

    mockInvoke.mockClear();

    const node = container.querySelector(
      "[data-segment='ui:navbar.board-selector']",
    ) as HTMLElement | null;
    expect(node).not.toBeNull();

    fireEvent.click(node!);

    const focusCalls = spatialFocusCalls();
    expect(focusCalls).toHaveLength(1);
    expect(focusCalls[0].fq).toBe(zone!.fq);

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
      "[data-segment='ui:navbar.inspect']",
    ) as HTMLElement | null;
    expect(node).not.toBeNull();

    fireEvent.click(node!);

    const focusCalls = spatialFocusCalls();
    expect(focusCalls).toHaveLength(1);
    expect(focusCalls[0].fq).toBe(leaf!.fq);

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
      "[data-segment='ui:navbar.search']",
    ) as HTMLElement | null;
    expect(node).not.toBeNull();

    fireEvent.click(node!);

    const focusCalls = spatialFocusCalls();
    expect(focusCalls).toHaveLength(1);
    expect(focusCalls[0].fq).toBe(leaf!.fq);

    unmount();
  });

  it("focus claim on the board-selector zone flips data-focused but renders no indicator", async () => {
    // The board-selector is a zone with `showFocus={false}` — its inner
    // leaves (the dropdown trigger and tear-off button registered by
    // <BoardSelector>, plus the editable name `<Field>` zone) own the
    // visible focus signal. The data-focused attribute still flips so
    // e2e selectors and debugging tooling can observe the claim, but no
    // `<FocusIndicator>` mounts on the zone wrapper itself.
    const { container, queryByTestId, unmount } = renderNavBar();
    await flushSetup();

    const zone = registerScopeArgs().find(
      (a) => a.segment === "ui:navbar.board-selector",
    )!;

    const node = container.querySelector(
      "[data-segment='ui:navbar.board-selector']",
    ) as HTMLElement;
    expect(node).not.toBeNull();
    expect(node.getAttribute("data-focused")).toBeNull();

    await fireFocusChanged({ next_fq: zone.fq as FullyQualifiedMoniker });

    await waitFor(() => {
      expect(node.getAttribute("data-focused")).not.toBeNull();
    });
    expect(queryByTestId("focus-indicator")).toBeNull();

    unmount();
  });

  it("focus claim mounts the FocusIndicator inside the inspect leaf", async () => {
    const { container, queryByTestId, unmount } = renderNavBar();
    await flushSetup();

    const leaf = registerScopeArgs().find(
      (a) => a.segment === "ui:navbar.inspect",
    )!;

    expect(queryByTestId("focus-indicator")).toBeNull();

    await fireFocusChanged({ next_fq: leaf.fq as FullyQualifiedMoniker });

    await waitFor(() => {
      expect(queryByTestId("focus-indicator")).not.toBeNull();
    });
    const node = container.querySelector(
      "[data-segment='ui:navbar.inspect']",
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

    await fireFocusChanged({ next_fq: leaf.fq as FullyQualifiedMoniker });

    await waitFor(() => {
      expect(queryByTestId("focus-indicator")).not.toBeNull();
    });
    const node = container.querySelector(
      "[data-segment='ui:navbar.search']",
    ) as HTMLElement;
    const indicator = queryByTestId("focus-indicator")!;
    expect(node.contains(indicator)).toBe(true);
    expect(node.getAttribute("data-focused")).not.toBeNull();

    unmount();
  });

  // -------------------------------------------------------------------------
  // Single-variant focus indicator
  //
  // The bar leaves wrap small icon buttons (24x24). The `<FocusIndicator>`
  // paints a 1px dotted border *inside* the host's box (`absolute inset-0
  // border border-dashed border-primary`), so it traces each focused
  // leaf's bounding box exactly without needing layout-side gap or
  // padding to make room for the decoration. There is no second visual
  // variant — one indicator, dotted inset everywhere.
  // -------------------------------------------------------------------------

  it("renders the dashed-inset border (not a ring) on the inspect leaf when focused", async () => {
    const { queryByTestId, unmount } = renderNavBar();
    await flushSetup();

    const leaf = registerScopeArgs().find(
      (a) => a.segment === "ui:navbar.inspect",
    )!;

    await fireFocusChanged({ next_fq: leaf.fq as FullyQualifiedMoniker });

    await waitFor(() => {
      expect(queryByTestId("focus-indicator")).not.toBeNull();
    });
    const indicator = queryByTestId("focus-indicator")!;
    // Dotted-inset signature: `absolute inset-0 border border-dashed
    // border-primary rounded-[inherit]`, NOT a `ring-2` outline. The
    // historic ring variant is gone — there is one and only one visual.
    expect(indicator.className).toContain("inset-0");
    expect(indicator.className).toContain("border");
    expect(indicator.className).toContain("border-dashed");
    expect(indicator.className).toContain("border-primary");
    expect(indicator.className).not.toContain("-left-2");
    expect(indicator.className).not.toContain("w-1");
    expect(indicator.className).not.toContain("ring-2");

    unmount();
  });

  it("renders the dashed-inset border (not a ring) on the search leaf when focused", async () => {
    const { queryByTestId, unmount } = renderNavBar();
    await flushSetup();

    const leaf = registerScopeArgs().find(
      (a) => a.segment === "ui:navbar.search",
    )!;

    await fireFocusChanged({ next_fq: leaf.fq as FullyQualifiedMoniker });

    await waitFor(() => {
      expect(queryByTestId("focus-indicator")).not.toBeNull();
    });
    const indicator = queryByTestId("focus-indicator")!;
    expect(indicator.className).toContain("inset-0");
    expect(indicator.className).toContain("border-dashed");
    expect(indicator.className).not.toContain("-left-2");

    unmount();
  });

  it("does not mount a focus indicator on the board-selector zone (leaves own the indicator)", async () => {
    // The board-selector zone uses `showFocus={false}` so the visible
    // bar is owned by inner leaves (dropdown trigger, tear-off button,
    // editable name Field). Confirm the zone-level focus claim does NOT
    // mount the indicator.
    const { queryByTestId, unmount } = renderNavBar();
    await flushSetup();

    const zone = registerScopeArgs().find(
      (a) => a.segment === "ui:navbar.board-selector",
    )!;

    await fireFocusChanged({ next_fq: zone.fq as FullyQualifiedMoniker });

    // Wait for the focus claim to propagate, then assert no indicator on
    // the zone.
    await new Promise((r) => setTimeout(r, 0));
    expect(queryByTestId("focus-indicator")).toBeNull();

    unmount();
  });

  // -------------------------------------------------------------------------
  // No outer ui:navbar FocusScope wrapper
  // -------------------------------------------------------------------------

  it("does NOT register an outer ui:navbar FocusScope wrapper", async () => {
    // Regression guard: a viewport-spanning `<FocusScope moniker="ui:navbar">`
    // around the bar swallows clicks landing on bar whitespace AND beam-search
    // candidates arriving from below — focus resolves to the parent rather
    // than to any inner leaf, so clicks on the board-name field, arrow-nav
    // from the left-nav, and arrow-nav from the perspective-bar all fail to
    // reach the inner leaves. The bar must stay a plain `<div role="banner">`.
    const { container, unmount } = renderNavBar();
    await flushSetup();

    const navbarZone = registerScopeArgs().find(
      (a) => a.segment === "ui:navbar",
    );
    expect(
      navbarZone,
      "ui:navbar wrapper must not register — see nav-bar.tsx docstring for the focus-swallowing rationale",
    ).toBeUndefined();

    expect(
      container.querySelector("[data-segment='ui:navbar']"),
      "no DOM element should carry data-segment='ui:navbar' — only inner leaves",
    ).toBeNull();

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
  // Inner scopes register as peer top-level scopes under the window layer
  //
  // With the outer `<FocusScope moniker="ui:navbar">` wrapper removed, the
  // bar's inner scopes (`ui:navbar.board-selector`, `ui:navbar.inspect`,
  // `ui:navbar.search`, and the percent-complete `<Field>` zone) register
  // as peer top-level scopes under the surrounding `<FocusLayer>`, just
  // like `ui:left-nav` and `ui:perspective-bar`. Their `parentZone` is
  // `null` and their `layerFq` points at the window layer. This is what
  // lets beam-search treat them as first-class navigation candidates from
  // peer chrome (left-nav, perspective-bar) without going through a
  // viewport-spanning parent that swallows hits.
  // -------------------------------------------------------------------------

  it("every inner navbar scope registers as a peer top-level scope (parentZone null) under the window layer", async () => {
    const { unmount } = renderNavBar();
    await flushSetup();

    const innerSegments = [
      "ui:navbar.board-selector",
      "ui:navbar.inspect",
      "ui:navbar.search",
      "field:board:b1.percent_complete",
    ] as const;

    for (const segment of innerSegments) {
      const reg = registerScopeArgs().find((a) => a.segment === segment);
      expect(reg, `${segment} must register`).toBeDefined();
      expect(
        reg!.parentZone,
        `${segment} must register as a peer top-level scope (parentZone === null), not under a removed ui:navbar wrapper`,
      ).toBeNull();
      expect(
        reg!.layerFq,
        `${segment} must publish a layerFq pointing at the window layer`,
      ).toBeTruthy();
    }

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
  // `spatial_register_scope`. None must be zero-width or zero-height: a
  // zero rect would silently break beam search and leave the user unable
  // to navigate, exactly the symptom the card is fixing.
  //
  // Coverage spans the inner peer-top-level scopes the navbar publishes:
  //   - `ui:navbar.board-selector`
  //   - `ui:navbar.inspect`
  //   - `ui:navbar.search`
  //   - `field:board:b1.percent_complete` (the percent-complete field zone)
  //
  // The outer `ui:navbar` wrapper was deliberately removed — it used to
  // swallow clicks and beam-search hits — so this test no longer checks
  // a wrapper rect.
  //
  // The Field mock above registers a real `<FocusScope>` so the field's
  // rect appears in the assertion alongside the leaves; mocking it as
  // a plain span would silently skip this regression guard.
  // -------------------------------------------------------------------------

  it("every navbar entry registers a non-zero rect at first paint", async () => {
    const { unmount } = renderNavBar();
    await flushSetup();

    /** Predicate: the entry's rect is positive on both axes. */
    const isPositiveRect = (entry: Record<string, unknown>): boolean => {
      const rect = entry.rect as { width: number; height: number } | undefined;
      if (!rect) return false;
      return rect.width > 0 && rect.height > 0;
    };

    const innerSegments = [
      "ui:navbar.board-selector",
      "ui:navbar.inspect",
      "ui:navbar.search",
      "field:board:b1.percent_complete",
    ] as const;

    for (const segment of innerSegments) {
      const entries = registerScopeArgs().filter((a) => a.segment === segment);
      expect(
        entries.length,
        `${segment} must register at first paint`,
      ).toBeGreaterThan(0);
      for (const entry of entries) {
        const rect = entry.rect as { width: number; height: number };
        expect(
          isPositiveRect(entry),
          `${segment} rect must be non-zero at first paint (got width=${rect?.width}, height=${rect?.height}); a zero rect silently breaks beam search and leaves the user unable to arrow-navigate to this entry`,
        ).toBe(true);
      }
    }

    unmount();
  });
});
