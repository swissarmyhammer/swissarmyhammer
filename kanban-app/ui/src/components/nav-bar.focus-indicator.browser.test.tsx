/**
 * Browser-mode tests for the navbar's focus-indicator wiring on each
 * sibling entry — the source-of-truth assertion for kanban card
 * `01KQ9XWHP2Y5H1QB5B3RJFEBBR`'s seam 1 (no visible indicator on a
 * focused navbar leaf).
 *
 * Mounts `<NavBar>` inside the production provider stack, drives a
 * `focus-changed` event for one navbar entry's key, then asserts:
 *
 *   1. The leaf's wrapper carries `data-focused="true"` (the spatial
 *      primitive's per-key claim subscription fired on the right key).
 *   2. A `<FocusIndicator>` (`[data-testid="focus-indicator"]`) is
 *      rendered as a descendant of that wrapper (the React state from
 *      step 1 reached the indicator's render path and the bar mounted).
 *
 * Both halves of the wiring must hold for the user to see the visible
 * cursor-bar on a focused navbar entry. Splitting them lets a regression
 * point at the right seam:
 *
 *   - Only `data-focused` flips → render-side bug (e.g. the bar's
 *     mounting effect was removed, or `<FocusIndicator>` was tree-
 *     shaken out of the build).
 *   - Neither flips → subscription bug (e.g. the leaf's `useFocusClaim`
 *     subscription was scoped to the wrong window or layer, or the
 *     event payload's `next_fq` didn't match the registered key).
 *   - Both flip but no `<FocusIndicator>` mounts → visible-bar wiring
 *     bug (e.g. `showFocusBar` was forced to `false` somewhere).
 *
 * # Test cases
 *
 * Five cases per the card's `Frontend tests` section:
 *
 *   1. `focus_indicator_renders_when_board_selector_leaf_is_focused`
 *   2. `focus_indicator_renders_when_inspect_leaf_is_focused`
 *   3. `focus_indicator_renders_when_search_leaf_is_focused`
 *   4. `focus_indicator_renders_when_percent_complete_field_zone_is_focused`
 *   5. `inspect_leaf_remount_does_not_lose_focus_indicator` — the
 *      conditional-render race regression guard. The inspect leaf is
 *      gated on `{board && <FocusScope>...}`. Toggling `board` from
 *      non-null → null → non-null mints a fresh `FullyQualifiedMoniker` on the
 *      remounted leaf; this test asserts the kernel can still focus
 *      the new key and the indicator appears on the remounted node.
 *
 * # Mock pattern
 *
 * Mirrors `nav-bar.spatial-nav.test.tsx` and
 * `focus-indicator.single-variant.spatial.test.tsx`:
 *   - `vi.hoisted` builds an `invoke` / `listen` mock pair the test owns.
 *   - `mockListen` records every `listen("focus-changed", cb)` callback
 *     so `fireFocusChanged(key)` can drive the React tree as if the Rust
 *     kernel had emitted a `focus-changed` event.
 *   - `<Field>` is mocked with a thin `<FocusZone>` wrapper so the
 *     percent-complete field's spatial registration runs against the
 *     same primitives production uses, without pulling in the entity
 *     store and field registries.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act, waitFor } from "@testing-library/react";
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
// `nav-bar.spatial-nav.test.tsx`.
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

// ---------------------------------------------------------------------------
// `<Field>` mock — wraps a `<FocusZone>` with the production moniker
// shape so the percent-complete field zone registers against the spatial
// graph without pulling in the full entity store and field registries.
// The wrapper preserves the production contract (the field IS a
// `<FocusZone>` whose moniker is `field:{type}:{id}.{name}`) so the
// indicator-render assertion runs against the same primitives the user
// hits at runtime.
// ---------------------------------------------------------------------------

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
        <FocusZone moniker={moniker} showFocusBar>
          <span data-testid="field-percent">{String(props.entityId)}</span>
        </FocusZone>
      );
    },
  };
});

// ---------------------------------------------------------------------------
// Imports — after mocks
// ---------------------------------------------------------------------------

import { useState } from "react";
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
 * `nav-bar.spatial-nav.test.tsx`.
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
 * Render `<NavBar>` inside the spatial-focus + window-root layer
 * providers that the production tree mounts in `App.tsx`.
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

describe("NavBar — focus-indicator renders on each navbar entry", () => {
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
  // 1. The board-selector is a zone (multi-leaf surface), not a leaf, so
  //    the data-focused attribute flips on the wrapper but no indicator
  //    mounts on the zone — its inner leaves own the visible focus signal
  //    (dropdown trigger, tear-off button, editable name Field).
  // -------------------------------------------------------------------------

  it("board_selector_zone_flips_data_focused_without_indicator", async () => {
    const { container, queryByTestId, unmount } = renderNavBar();
    await flushSetup();

    const zone = registerZoneArgs().find(
      (a) => a.segment === "ui:navbar.board-selector",
    );
    expect(zone, "board-selector zone must register").toBeDefined();

    expect(queryByTestId("focus-indicator")).toBeNull();

    await fireFocusChanged({ next_fq: zone!.fq as FullyQualifiedMoniker });

    await waitFor(() => {
      const node = container.querySelector(
        "[data-segment='ui:navbar.board-selector']",
      ) as HTMLElement | null;
      expect(node).not.toBeNull();
      expect(node!.getAttribute("data-focused")).not.toBeNull();
    });

    // No indicator on the zone wrapper — leaves own the visible focus.
    expect(queryByTestId("focus-indicator")).toBeNull();

    unmount();
  });

  // -------------------------------------------------------------------------
  // 2. Indicator renders on the inspect leaf (when board is non-null).
  // -------------------------------------------------------------------------

  it("focus_indicator_renders_when_inspect_leaf_is_focused", async () => {
    const { container, queryByTestId, unmount } = renderNavBar();
    await flushSetup();

    const leaf = registerScopeArgs().find(
      (a) => a.segment === "ui:navbar.inspect",
    );
    expect(leaf, "inspect leaf must register when board is loaded").toBeDefined();

    expect(queryByTestId("focus-indicator")).toBeNull();

    await fireFocusChanged({ next_fq: leaf!.fq as FullyQualifiedMoniker });

    await waitFor(() => {
      const node = container.querySelector(
        "[data-segment='ui:navbar.inspect']",
      ) as HTMLElement | null;
      expect(node).not.toBeNull();
      expect(node!.getAttribute("data-focused")).not.toBeNull();
    });

    const node = container.querySelector(
      "[data-segment='ui:navbar.inspect']",
    ) as HTMLElement;
    const indicator = queryByTestId("focus-indicator");
    expect(indicator).not.toBeNull();
    expect(node.contains(indicator!)).toBe(true);

    unmount();
  });

  // -------------------------------------------------------------------------
  // 3. Indicator renders on the search leaf.
  // -------------------------------------------------------------------------

  it("focus_indicator_renders_when_search_leaf_is_focused", async () => {
    const { container, queryByTestId, unmount } = renderNavBar();
    await flushSetup();

    const leaf = registerScopeArgs().find(
      (a) => a.segment === "ui:navbar.search",
    );
    expect(leaf, "search leaf must register").toBeDefined();

    expect(queryByTestId("focus-indicator")).toBeNull();

    await fireFocusChanged({ next_fq: leaf!.fq as FullyQualifiedMoniker });

    await waitFor(() => {
      const node = container.querySelector(
        "[data-segment='ui:navbar.search']",
      ) as HTMLElement | null;
      expect(node).not.toBeNull();
      expect(node!.getAttribute("data-focused")).not.toBeNull();
    });

    const node = container.querySelector(
      "[data-segment='ui:navbar.search']",
    ) as HTMLElement;
    const indicator = queryByTestId("focus-indicator");
    expect(indicator).not.toBeNull();
    expect(node.contains(indicator!)).toBe(true);

    unmount();
  });

  // -------------------------------------------------------------------------
  // 4. Indicator renders on the percent-complete field zone.
  //
  // The field zone is registered via `spatial_register_zone` (it's a
  // `<FocusZone>`, not a `<FocusScope>`). Its moniker is
  // `field:board:b1.percent_complete`. The mocked `<Field>` opts in to
  // `showFocusBar` so the indicator mounts when the zone is focused —
  // matching the production wiring that lets the per-field bar tell the
  // user which atom of the row carries focus.
  // -------------------------------------------------------------------------

  it("focus_indicator_renders_when_percent_complete_field_zone_is_focused", async () => {
    const { container, queryByTestId, unmount } = renderNavBar();
    await flushSetup();

    const zone = registerZoneArgs().find(
      (a) => a.segment === "field:board:b1.percent_complete",
    );
    expect(
      zone,
      "percent-complete field zone must register inside the navbar",
    ).toBeDefined();

    expect(queryByTestId("focus-indicator")).toBeNull();

    await fireFocusChanged({ next_fq: zone!.fq as FullyQualifiedMoniker });

    await waitFor(() => {
      const node = container.querySelector(
        "[data-segment='field:board:b1.percent_complete']",
      ) as HTMLElement | null;
      expect(node).not.toBeNull();
      expect(node!.getAttribute("data-focused")).not.toBeNull();
    });

    const node = container.querySelector(
      "[data-segment='field:board:b1.percent_complete']",
    ) as HTMLElement;
    const indicator = queryByTestId("focus-indicator");
    expect(indicator).not.toBeNull();
    expect(node.contains(indicator!)).toBe(true);

    unmount();
  });

  // -------------------------------------------------------------------------
  // 5. Conditional re-mount of the inspect leaf does not lose the
  //    focus-indicator wiring.
  //
  // The inspect leaf is gated on `{board && <FocusScope>...}`. When
  // `board` flips from non-null → null → non-null, the leaf unmounts
  // and re-mounts, minting a fresh `FullyQualifiedMoniker` on the new mount
  // (`<FocusScope>`'s `useRef(crypto.randomUUID())`). The kernel's
  // `focused_key` may still point at the old key after an unmount; the
  // regression guard is that focusing the **fresh** key produces a
  // visible indicator on the **new** wrapper.
  //
  // This is the same shape as the click-to-focus race in card
  // `01KQ9XBAG5P9W3JREQYNGAYM8Y`. The guard pins that the conditional
  // re-mount path does not silently break the indicator wiring.
  // -------------------------------------------------------------------------

  it("inspect_leaf_remount_does_not_lose_focus_indicator", async () => {
    function Harness() {
      const [board, setBoard] = useState<BoardData | null>(MOCK_BOARD);
      // Drive the mock so `useBoardData()` reflects the local state.
      mockBoardData.mockImplementation(() => board);
      return (
        <SpatialFocusProvider>
          <FocusLayer name={WINDOW_LAYER_NAME}>
            <TooltipProvider delayDuration={100}>
              <NavBar />
            </TooltipProvider>
            <button
              data-testid="toggle-board"
              type="button"
              onClick={() => setBoard((b) => (b ? null : MOCK_BOARD))}
            >
              toggle
            </button>
          </FocusLayer>
        </SpatialFocusProvider>
      );
    }

    const { container, queryByTestId, getByTestId, unmount } = render(
      <Harness />,
    );
    await flushSetup();

    // Focus the inspect leaf on the first mount and confirm the indicator
    // appears — establishes the baseline wiring before the remount.
    const firstInspect = registerScopeArgs().find(
      (a) => a.segment === "ui:navbar.inspect",
    );
    expect(firstInspect, "inspect leaf must register on first mount").toBeDefined();

    await fireFocusChanged({ next_fq: firstInspect!.fq as FullyQualifiedMoniker });
    await waitFor(() => {
      const node = container.querySelector(
        "[data-segment='ui:navbar.inspect']",
      ) as HTMLElement | null;
      expect(node).not.toBeNull();
      expect(node!.getAttribute("data-focused")).not.toBeNull();
    });
    expect(queryByTestId("focus-indicator")).not.toBeNull();

    // Snapshot the count of inspect-leaf register calls so we can detect
    // the remount minted a fresh key.
    const beforeRemountInspectCount = registerScopeArgs().filter(
      (a) => a.segment === "ui:navbar.inspect",
    ).length;

    // Flip board → null. The inspect leaf unmounts; the kernel still
    // believes inspect.fq is focused, but the wrapper is gone.
    await act(async () => {
      getByTestId("toggle-board").click();
      await Promise.resolve();
    });
    await flushSetup();

    expect(
      container.querySelector("[data-segment='ui:navbar.inspect']"),
      "inspect leaf must unmount when board flips to null",
    ).toBeNull();

    // Flip board back → non-null. The inspect leaf re-mounts. A fresh
    // `<FocusScope>` mints a new FullyQualifiedMoniker on its `useRef`, so the
    // kernel sees a new register call with a new key.
    await act(async () => {
      getByTestId("toggle-board").click();
      await Promise.resolve();
    });
    await flushSetup();

    const allInspectRegistrations = registerScopeArgs().filter(
      (a) => a.segment === "ui:navbar.inspect",
    );
    expect(
      allInspectRegistrations.length,
      "inspect leaf must register a second time on remount",
    ).toBeGreaterThan(beforeRemountInspectCount);

    const remountedInspect =
      allInspectRegistrations[allInspectRegistrations.length - 1];
    // Under the path-monikers identity model the FQM is deterministic
    // (`<parent-fq>/<segment>`) — remount of the same primitive in the
    // same parent path produces the SAME FQM. The fresh-UUID semantics
    // from the legacy `crypto.randomUUID()` model no longer apply; what
    // we care about is that a register call fired again so the kernel
    // restored the entry, which the count check above already pins.
    expect(
      remountedInspect.fq,
      "remount under path-monikers re-uses the deterministic FQM",
    ).toBe(firstInspect!.fq);

    // The user lands on the remounted leaf via `spatial_focus(newKey)`.
    // The kernel emits `focus-changed` with `next_fq = remounted.fq`,
    // and the new leaf's `useFocusClaim` subscription should fire,
    // mounting the indicator on the new wrapper.
    await fireFocusChanged({ next_fq: remountedInspect.fq as FullyQualifiedMoniker });

    await waitFor(() => {
      const node = container.querySelector(
        "[data-segment='ui:navbar.inspect']",
      ) as HTMLElement | null;
      expect(node).not.toBeNull();
      expect(node!.getAttribute("data-focused")).not.toBeNull();
    });

    const node = container.querySelector(
      "[data-segment='ui:navbar.inspect']",
    ) as HTMLElement;
    const indicator = queryByTestId("focus-indicator");
    expect(
      indicator,
      "indicator must mount on the remounted inspect leaf when its fresh key is focused",
    ).not.toBeNull();
    expect(
      node.contains(indicator!),
      "indicator must live inside the new inspect wrapper, not the unmounted one",
    ).toBe(true);

    unmount();
  });
});
