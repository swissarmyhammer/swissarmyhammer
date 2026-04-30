/**
 * Browser-mode tests for the perspective tab bar's focus-indicator wiring
 * on each sibling tab leaf — the source-of-truth assertion for kanban
 * card `01KQ9Z56M556DQHYMA502B9FKB`'s seam 1 (no visible indicator on a
 * focused perspective tab).
 *
 * Mounts `<PerspectiveTabBar>` inside the production provider stack,
 * drives a `focus-changed` event for one perspective tab leaf's key,
 * then asserts:
 *
 *   1. The leaf's wrapper carries `data-focused="true"` (the spatial
 *      primitive's per-key claim subscription fired on the right key).
 *   2. A `<FocusIndicator>` (`[data-testid="focus-indicator"]`) is
 *      rendered as a descendant of that wrapper (the React state from
 *      step 1 reached the indicator's render path).
 *
 * Both halves of the wiring must hold for the user to see the visible
 * cursor-bar on a focused perspective tab. Splitting them lets a
 * regression point at the right seam:
 *
 *   - Only `data-focused` flips → render-side bug (e.g. the indicator
 *     was tree-shaken out of the build, or `<FocusIndicator>` no longer
 *     mounts on the leaf).
 *   - Neither flips → subscription bug (e.g. the leaf's `useFocusClaim`
 *     subscription was scoped to the wrong layer, or the event payload's
 *     `next_fq` didn't match the registered key).
 *   - Both flip but no `<FocusIndicator>` mounts → visible-bar wiring
 *     bug (e.g. `showFocusBar` was forced to `false` somewhere on the
 *     leaf).
 *
 * # Surface specifics that differ from the navbar
 *
 * The perspective bar has wrinkles the navbar does not:
 *
 *   - **Active-tab inline chrome**: the active tab renders extra
 *     `<FilterFocusButton>` and `<GroupPopoverButton>` siblings inside
 *     the same `<FocusScope>` wrapper, growing the leaf's bounding rect.
 *     Test #2 pins that the indicator still mounts on the active tab
 *     (the rect growth does not interfere with the indicator's
 *     containing block / overflow).
 *   - **Active-tab change does NOT remount the leaf**: clicking an
 *     inactive tab to make it active flips the `isActive` flag inside
 *     the unchanged `<FocusScope>` wrapper. Test #3 pins that the same
 *     wrapper still reports `data-focused="true"` and renders the
 *     indicator after activation — there is no FullyQualifiedMoniker churn from
 *     tab activation, unlike the navbar's inspect-leaf
 *     `{board && ...}` conditional.
 *   - **Inline rename editor**: when a tab is in rename mode, the
 *     `TabButton` renders `<InlineRenameEditor>` in place of the name.
 *     The editor takes DOM focus directly. Tests #4 and #5 pin that
 *     focus and the indicator return to the leaf's wrapper after the
 *     rename commits (Enter) or cancels (Escape).
 *
 * # Test cases
 *
 * Five cases per the card's `Frontend tests` section:
 *
 *   1. `focus_indicator_renders_when_inactive_tab_is_focused`
 *   2. `focus_indicator_renders_when_active_tab_is_focused`
 *   3. `focus_indicator_persists_through_tab_activation`
 *   4. `focus_indicator_returns_after_rename_commit`
 *   5. `focus_indicator_returns_after_rename_cancel`
 *
 * # Mock pattern
 *
 * Mirrors `perspective-tab-bar.enter-rename.spatial.test.tsx` and
 * `nav-bar.focus-indicator.browser.test.tsx`:
 *   - `vi.hoisted` builds an `invoke` / `listen` mock pair the test
 *     owns.
 *   - `mockListen` records every `listen("focus-changed", cb)` callback
 *     so `fireFocusChanged(key)` can drive the React tree as if the
 *     Rust kernel had emitted a `focus-changed` event.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act, waitFor } from "@testing-library/react";
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
// Perspective + view + UI mocks — match the shape used by
// `perspective-tab-bar.enter-rename.spatial.test.tsx`.
// ---------------------------------------------------------------------------

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
  PerspectiveProvider: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
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

const mockUIState = () => ({
  keymap_mode: "cua" as const,
  scope_chain: [],
  open_boards: [],
  has_clipboard: false,
  clipboard_entity_type: null,
  windows: { main: { palette_open: false, palette_mode: "command" } },
  recent_boards: [],
});

vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => mockUIState(),
  useUIStateLoading: () => ({ state: mockUIState(), loading: false }),
  UIStateProvider: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
}));

// ---------------------------------------------------------------------------
// Imports — after mocks
// ---------------------------------------------------------------------------

import { PerspectiveTabBar, triggerStartRename } from "./perspective-tab-bar";
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
  // Two ticks: first lets `useEffect` callbacks run, second lets any
  // Promise-resolution-driven follow-on (e.g. `subscribeFocusChanged`'s
  // listener registration) settle.
  await act(async () => {
    await Promise.resolve();
  });
  await act(async () => {
    await Promise.resolve();
  });
}

/**
 * Drive a `focus-changed` event into the React tree as if the Rust kernel
 * had emitted one for the current window.
 */
async function fireFocusChanged({
  prev_fq = null,
  next_fq = null,
  next_segment = null,
}: {
  prev_fq?: FullyQualifiedMoniker | null;
  next_fq?: FullyQualifiedMoniker | null;
  next_segment?: string | null;
}) {
  const payload: FocusChangedPayload = {
    window_label: "main" as WindowLabel,
    prev_fq,
    next_fq,
    next_segment: next_segment as FocusChangedPayload["next_segment"],
  };
  const handlers = listeners.get("focus-changed") ?? [];
  await act(async () => {
    for (const handler of handlers) handler({ payload });
    await Promise.resolve();
  });
}

/**
 * Render the perspective tab bar inside the production-shaped provider
 * stack — `<SpatialFocusProvider>` + `<FocusLayer name="window">` so the
 * spatial-nav primitives mount their full chrome (per-key claim
 * subscription, indicator render, register/unregister).
 */
function renderPerspectiveBar() {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={WINDOW_LAYER_NAME}>
        <TooltipProvider delayDuration={100}>
          <PerspectiveTabBar />
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

/** Find the registered `FullyQualifiedMoniker` for a perspective tab moniker. */
function findTabKey(perspectiveId: string): FullyQualifiedMoniker | undefined {
  const scope = registerScopeArgs().find(
    (a) => a.segment === `perspective_tab:${perspectiveId}`,
  );
  return scope?.fq as FullyQualifiedMoniker | undefined;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("PerspectiveTabBar — focus-indicator renders on each tab leaf", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    // Three perspectives — p2 active, mirroring the Rust fixture's
    // three-tab perspective bar (p1, p2 active/wider, p3). The
    // active-tab rect growth (caused by inline `<FilterFocusButton>` +
    // `<GroupPopoverButton>` chrome) is exercised at the React level by
    // test #2 below.
    mockPerspectivesValue = {
      perspectives: [
        { id: "p1", name: "Sprint", view: "board" },
        { id: "p2", name: "Backlog", view: "board" },
        { id: "p3", name: "Archive", view: "board" },
      ],
      activePerspective: { id: "p2", name: "Backlog", view: "board" },
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
  // 1. Indicator renders on an inactive perspective tab.
  // -------------------------------------------------------------------------

  it("focus_indicator_renders_when_inactive_tab_is_focused", async () => {
    const { container, queryByTestId, unmount } = renderPerspectiveBar();
    await flushSetup();

    // p1 is inactive (p2 is the active perspective). Locate its
    // registered FullyQualifiedMoniker and drive a focus-changed event to it.
    const p1Key = findTabKey("p1");
    expect(p1Key, "perspective_tab:p1 leaf must register").toBeDefined();

    // No indicator should render before focus moves to a tab leaf.
    expect(
      container.querySelector(
        "[data-segment='perspective_tab:p1'] [data-testid='focus-indicator']",
      ),
    ).toBeNull();

    await fireFocusChanged({
      next_fq: p1Key!,
      next_segment: asSegment("perspective_tab:p1"),
    });

    await waitFor(() => {
      const node = container.querySelector(
        "[data-segment='perspective_tab:p1']",
      ) as HTMLElement | null;
      expect(node).not.toBeNull();
      expect(node!.getAttribute("data-focused")).toBe("true");
    });

    const node = container.querySelector(
      "[data-segment='perspective_tab:p1']",
    ) as HTMLElement;
    const indicator = queryByTestId("focus-indicator");
    expect(
      indicator,
      "indicator must mount when an inactive tab leaf is focused",
    ).not.toBeNull();
    expect(
      node.contains(indicator!),
      "indicator must render inside the focused inactive tab's wrapper",
    ).toBe(true);

    unmount();
  });

  // -------------------------------------------------------------------------
  // 2. Indicator renders on the active perspective tab.
  //
  // Pins that the active-tab inline chrome
  // (`<FilterFocusButton>` + `<GroupPopoverButton>`) does not interfere
  // with the indicator's containing block / overflow. The active tab's
  // `<FocusScope>` wraps the entire `<div className="inline-flex
  // items-center">` (see `perspective-tab-bar.tsx` `PerspectiveTab`)
  // and the wrapper has `position: relative` from the `<FocusScope>`
  // primitive's className merge — so the absolutely-positioned
  // indicator's `-left-2` placement still resolves against the
  // wrapper, not the surrounding bar.
  // -------------------------------------------------------------------------

  it("focus_indicator_renders_when_active_tab_is_focused", async () => {
    const { container, queryByTestId, unmount } = renderPerspectiveBar();
    await flushSetup();

    // p2 is active. Its leaf wrapper has an extra
    // `<FilterFocusButton>` + `<GroupPopoverButton>` rendered next to
    // the `TabButton`, growing the leaf's rect.
    const p2Key = findTabKey("p2");
    expect(p2Key, "perspective_tab:p2 leaf must register").toBeDefined();

    expect(
      container.querySelector(
        "[data-segment='perspective_tab:p2'] [data-testid='focus-indicator']",
      ),
    ).toBeNull();

    await fireFocusChanged({
      next_fq: p2Key!,
      next_segment: asSegment("perspective_tab:p2"),
    });

    await waitFor(() => {
      const node = container.querySelector(
        "[data-segment='perspective_tab:p2']",
      ) as HTMLElement | null;
      expect(node).not.toBeNull();
      expect(node!.getAttribute("data-focused")).toBe("true");
    });

    const node = container.querySelector(
      "[data-segment='perspective_tab:p2']",
    ) as HTMLElement;
    const indicator = queryByTestId("focus-indicator");
    expect(
      indicator,
      "indicator must mount when the active (wider) tab leaf is focused",
    ).not.toBeNull();
    expect(
      node.contains(indicator!),
      "indicator must render inside the active tab's wrapper, not in the bar's chrome",
    ).toBe(true);

    unmount();
  });

  // -------------------------------------------------------------------------
  // 3. Indicator persists through tab activation.
  //
  // Pin that the `isActive` flag flips render content (extra inline
  // chrome appears) but not the wrapping `<FocusScope>` leaf. The
  // FullyQualifiedMoniker on a registered leaf is held in a `useRef` minted once
  // on mount; activation does not unmount the leaf, so the same key
  // remains focused through the activation cycle.
  //
  // Regression guard for the navbar's `{board && ...}` shape — the
  // perspective bar deliberately does NOT have that pattern, and this
  // test ensures it stays that way. If a future edit introduces a
  // conditional remount (e.g. `{isActive ? <FocusScope> ... : null}`),
  // this test fails.
  // -------------------------------------------------------------------------

  it("focus_indicator_persists_through_tab_activation", async () => {
    const { container, queryByTestId, rerender, unmount } =
      renderPerspectiveBar();
    await flushSetup();

    // Focus inactive p1.
    const p1Key = findTabKey("p1");
    expect(p1Key, "perspective_tab:p1 leaf must register").toBeDefined();

    await fireFocusChanged({
      next_fq: p1Key!,
      next_segment: asSegment("perspective_tab:p1"),
    });

    await waitFor(() => {
      const node = container.querySelector(
        "[data-segment='perspective_tab:p1']",
      ) as HTMLElement | null;
      expect(node).not.toBeNull();
      expect(node!.getAttribute("data-focused")).toBe("true");
    });
    expect(
      queryByTestId("focus-indicator"),
      "indicator must mount on inactive p1 before activation",
    ).not.toBeNull();

    // Snapshot the count of p1 register calls. After activation, the
    // count must NOT grow — the leaf must not unmount + remount.
    const beforeActivationCount = registerScopeArgs().filter(
      (a) => a.segment === "perspective_tab:p1",
    ).length;

    // Activate p1 by flipping the mock perspective context. Re-render
    // forces the React tree to re-evaluate the `isActive` prop on every
    // tab.
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      activePerspective: { id: "p1", name: "Sprint", view: "board" },
    };
    rerender(
      <SpatialFocusProvider>
        <FocusLayer name={WINDOW_LAYER_NAME}>
          <TooltipProvider delayDuration={100}>
            <PerspectiveTabBar />
          </TooltipProvider>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const afterActivationCount = registerScopeArgs().filter(
      (a) => a.segment === "perspective_tab:p1",
    ).length;
    expect(
      afterActivationCount,
      "activating a tab must NOT remount its <FocusScope> leaf — the FullyQualifiedMoniker stays stable",
    ).toBe(beforeActivationCount);

    // The same wrapper still reports `data-focused="true"` and the
    // indicator is still rendered inside it — no FullyQualifiedMoniker churn means
    // the kernel's focused_key still points at the same leaf.
    const node = container.querySelector(
      "[data-segment='perspective_tab:p1']",
    ) as HTMLElement;
    expect(node).not.toBeNull();
    expect(node.getAttribute("data-focused")).toBe("true");

    const indicator = queryByTestId("focus-indicator");
    expect(
      indicator,
      "indicator must remain mounted after tab activation",
    ).not.toBeNull();
    expect(
      node.contains(indicator!),
      "indicator must remain inside the now-active tab's wrapper",
    ).toBe(true);

    unmount();
  });

  // -------------------------------------------------------------------------
  // 4. Indicator returns after rename commit.
  //
  // The active perspective is p2. Triggering `ui.entity.startRename`
  // mounts the `<InlineRenameEditor>` inside the `TabButton`, replacing
  // the name text with a CM6 editor that takes DOM focus directly.
  // When the rename commits (Enter), the editor unmounts and the
  // `TabButton` reverts to plain text.
  //
  // The `<FocusScope>` wrapper around the tab is unchanged through the
  // entire rename round-trip — the rename mode flips render content
  // inside the `TabButton`, not the wrapping leaf. So the leaf's
  // FullyQualifiedMoniker stays stable, the kernel's focused_key never moves, and
  // the indicator remains mounted on the wrapper throughout.
  //
  // Pin via test: focus p2, drive rename via `triggerStartRename`,
  // commit by dispatching Enter on the CM6 content, assert the
  // indicator is back on the leaf wrapper.
  // -------------------------------------------------------------------------

  it("focus_indicator_returns_after_rename_commit", async () => {
    const { container, queryByTestId, unmount } = renderPerspectiveBar();
    await flushSetup();

    // Focus active p2.
    const p2Key = findTabKey("p2");
    expect(p2Key).toBeDefined();

    await fireFocusChanged({
      next_fq: p2Key!,
      next_segment: asSegment("perspective_tab:p2"),
    });

    await waitFor(() => {
      const node = container.querySelector(
        "[data-segment='perspective_tab:p2']",
      ) as HTMLElement | null;
      expect(node?.getAttribute("data-focused")).toBe("true");
    });
    expect(
      queryByTestId("focus-indicator"),
      "indicator must mount on the focused active tab before rename starts",
    ).not.toBeNull();

    // Enter rename mode via the same module-level broadcaster the
    // AppShell global command uses. This mounts `<InlineRenameEditor>`
    // inside the `TabButton`.
    await act(async () => {
      triggerStartRename();
      await Promise.resolve();
    });

    // The CM6 rename editor mounts inside the active tab's wrapper.
    await waitFor(() => {
      const renameEditor = container.querySelector(
        "[data-segment='perspective_tab:p2'] .cm-editor",
      );
      expect(renameEditor).not.toBeNull();
    });

    // Commit the rename by dispatching Enter on the CM6 content. The
    // editor unmounts and the wrapper reverts to the plain name text.
    const cmContent = container.querySelector(
      "[data-segment='perspective_tab:p2'] .cm-content",
    ) as HTMLElement;
    expect(cmContent).not.toBeNull();
    await act(async () => {
      cmContent.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "Enter",
          bubbles: true,
          cancelable: true,
        }),
      );
      await new Promise((r) => setTimeout(r, 50));
    });
    await flushSetup();

    // Rename editor is gone; the leaf wrapper still carries
    // `data-focused="true"` and the indicator is rendered inside it —
    // the wrapping `<FocusScope>` was not remounted by the rename
    // round-trip.
    expect(
      container.querySelector(
        "[data-segment='perspective_tab:p2'] .cm-editor",
      ),
      "rename editor must unmount after commit",
    ).toBeNull();

    const node = container.querySelector(
      "[data-segment='perspective_tab:p2']",
    ) as HTMLElement;
    expect(node).not.toBeNull();
    expect(node.getAttribute("data-focused")).toBe("true");

    const indicator = queryByTestId("focus-indicator");
    expect(
      indicator,
      "indicator must return to the active tab's wrapper after rename commit",
    ).not.toBeNull();
    expect(
      node.contains(indicator!),
      "indicator must live inside the active tab's wrapper after the rename round-trip",
    ).toBe(true);

    unmount();
  });

  // -------------------------------------------------------------------------
  // 5. Indicator returns after rename cancel.
  //
  // Same as test #4 but with Escape (cancel) instead of Enter (commit).
  // The two paths must converge on the same outcome — leaf wrapper
  // retains focus, indicator remains mounted — because both unmount
  // the inner `<InlineRenameEditor>` without affecting the wrapping
  // `<FocusScope>`.
  // -------------------------------------------------------------------------

  it("focus_indicator_returns_after_rename_cancel", async () => {
    const { container, queryByTestId, unmount } = renderPerspectiveBar();
    await flushSetup();

    const p2Key = findTabKey("p2");
    expect(p2Key).toBeDefined();

    await fireFocusChanged({
      next_fq: p2Key!,
      next_segment: asSegment("perspective_tab:p2"),
    });

    await waitFor(() => {
      const node = container.querySelector(
        "[data-segment='perspective_tab:p2']",
      ) as HTMLElement | null;
      expect(node?.getAttribute("data-focused")).toBe("true");
    });
    expect(queryByTestId("focus-indicator")).not.toBeNull();

    await act(async () => {
      triggerStartRename();
      await Promise.resolve();
    });

    await waitFor(() => {
      const renameEditor = container.querySelector(
        "[data-segment='perspective_tab:p2'] .cm-editor",
      );
      expect(renameEditor).not.toBeNull();
    });

    // Cancel via Escape. In cua mode this dismisses without dispatching
    // `perspective.rename`; the editor unmounts the same way Enter
    // does.
    const cmContent = container.querySelector(
      "[data-segment='perspective_tab:p2'] .cm-content",
    ) as HTMLElement;
    await act(async () => {
      cmContent.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "Escape",
          bubbles: true,
          cancelable: true,
        }),
      );
      await new Promise((r) => setTimeout(r, 50));
    });
    await flushSetup();

    expect(
      container.querySelector(
        "[data-segment='perspective_tab:p2'] .cm-editor",
      ),
      "rename editor must unmount after cancel",
    ).toBeNull();

    const node = container.querySelector(
      "[data-segment='perspective_tab:p2']",
    ) as HTMLElement;
    expect(node).not.toBeNull();
    expect(node.getAttribute("data-focused")).toBe("true");

    const indicator = queryByTestId("focus-indicator");
    expect(
      indicator,
      "indicator must return to the active tab's wrapper after rename cancel",
    ).not.toBeNull();
    expect(
      node.contains(indicator!),
      "indicator must live inside the active tab's wrapper after the rename round-trip",
    ).toBe(true);

    unmount();
  });
});
