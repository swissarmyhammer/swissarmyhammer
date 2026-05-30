/**
 * Regression tests for the Filter tab-button migration to command-driven
 * rendering.
 *
 * **History:**
 *
 *   - Initial migration (task `01KRE1YA65MMG29RDQDQ0VPJQG`) replaced the
 *     hardcoded `<FilterFocusButton>` with a registry-rendered
 *     `<CommandButton>` driven by the `perspective.filter.focus` YAML
 *     entry. That migration invented a parallel focus channel: the
 *     click dispatched `perspective.filter.focus`, the backend
 *     `FocusFilterCmd` returned a `FocusFilter` marker envelope, the
 *     Tauri dispatcher emitted a `ui.focus.filter` event, and
 *     `<FilterEditorBody>` listened for it and imperatively focused
 *     the CM6 editor.
 *
 *   - Rewire to `nav.focus` (task `01KRGZY33P99J7CGG0XRQGZ352`):
 *     deleted the parallel channel and routed the click through the
 *     spatial-nav focus primitive. Now the click dispatches
 *     `nav.focus({ args: { fq: <filter_editor FQM> } })`, which is the
 *     same path that every other focus claim in the app goes through
 *     (jump-to, arrow nav, palette focus, etc.). The kernel emits
 *     `focus-changed` back to React; the `filter_editor:${id}` scope's
 *     `nav.drillIn` (Enter) drives the CM6 editor to take editing focus.
 *
 * Contracts pinned by this file:
 *
 *   1. Clicking the Filter `<CommandButton>` dispatches `nav.focus` with
 *      `args.fq` ending in `filter_editor:<active-perspective-id>`.
 *   2. Switching the active perspective and then clicking Filter lands
 *      focus on the NEW active perspective's `filter_editor` moniker —
 *      not on the previously active one.
 *   3. The button carries the `text-primary` highlight when
 *      `perspective.filter` is set.
 *   4. The button does NOT carry the highlight when
 *      `perspective.filter` is unset/empty.
 *   5. The button registers the spatial-nav moniker
 *      `perspective_tab.perspective.filter.focus:{id}` — the legacy
 *      `perspective_tab.filter:{id}` is gone.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act, fireEvent } from "@testing-library/react";
import { TooltipProvider } from "@/components/ui/tooltip";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before any module imports that pull
// command-scope / filter-editor / perspective-tab-bar.
// ---------------------------------------------------------------------------

type ListenCallback = (event: { payload: unknown }) => void;

const { mockInvoke, mockListen, listeners } = vi.hoisted(() => {
  const listeners = new Map<string, ListenCallback[]>();
  const mockInvoke = vi.fn(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (..._args: any[]): Promise<unknown> => Promise.resolve(null),
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
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (...a: Parameters<typeof mockListen>) => mockListen(...a),
  emit: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
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
// Domain context mocks — same shape as the existing perspective-tab-bar
// test files so this file feels at home next to its siblings.
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

// Tab buttons source from the Command registry via `useCommandList`.
let mockRegistry: Array<Record<string, unknown>> = [];
vi.mock("@/hooks/use-command-list", () => ({
  useCommandList: () => ({
    commands: mockRegistry,
    loading: false,
    refresh: vi.fn(),
  }),
}));

vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({ getEntities: () => [] }),
}));

let mockBoardId = "test-board";
vi.mock("@/components/window-container", () => ({
  useBoardData: () => ({
    board: {
      entity_type: "board",
      id: mockBoardId,
      moniker: `board:${mockBoardId}`,
      fields: {},
    },
    virtualTagMeta: [],
  }),
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
  windows: {},
  recent_boards: [],
});

vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => mockUIState(),
  useUIStateLoading: () => ({ state: mockUIState(), loading: false }),
}));

// ---------------------------------------------------------------------------
// Component-under-test imports — must come AFTER `vi.mock` above.
// ---------------------------------------------------------------------------

import { PerspectiveTabBar } from "./perspective-tab-bar";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { FocusLayer } from "./focus-layer";
import { asSegment } from "@/types/spatial";

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/** Publish `commands` through the `useCommandList` seam (perspective-scoped). */
function mockResolvedCommands(
  commands: Array<{
    id: string;
    name: string;
    tab_button?: { icon: string };
    params?: readonly { name: string; from?: string }[];
    keys?: Record<string, string> | Record<string, never>;
  }>,
) {
  mockRegistry = commands.map((c) => ({ scope: ["entity:perspective"], ...c }));
}

/** Render `<PerspectiveTabBar>` inside the standard provider stack. */
function renderTabBar() {
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

/**
 * Wait for `list_commands_for_scope`'s async effect to settle.
 *
 * `<RegistryTabButtons>` calls `invoke` inside a `useEffect` and writes
 * the result via `setCommands`. Three event-loop turns reliably cover
 * resolve → setState → register effect.
 */
async function flushEffects() {
  await act(async () => {
    for (let i = 0; i < 3; i += 1) {
      // eslint-disable-next-line no-await-in-loop
      await new Promise<void>((resolve) => setTimeout(resolve, 0));
    }
  });
}

/** Build a registry payload for the new `perspective.filter.focus` command. */
function focusFilterRegistryEntry() {
  return {
    id: "perspective.filter.focus",
    name: "Focus Filter",
    tab_button: { icon: "filter" },
    // `params` is intentionally `scope_chain`-only: clicking dispatches
    // immediately, no popover. This mirrors the YAML in
    // `swissarmyhammer-kanban/builtin/commands/perspective.yaml`.
    params: [{ name: "perspective_id", from: "scope_chain" }],
    keys: {},
  };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("perspective-tab-bar — Filter command migration", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockListen.mockClear();
    listeners.clear();
    mockRegistry = [];
    mockBoardId = "test-board";
    mockPerspectivesValue = {
      perspectives: [{ id: "p1", name: "Sprint", view: "board" }],
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

  // -------------------------------------------------------------------------
  // 1. Clicking the Filter `<CommandButton>` routes through `nav.focus` with
  //    `args.fq` ending in `filter_editor:<active-perspective-id>`.
  //
  // `nav.focus` is registered as a frontend-execute handler in
  // `<SpatialFocusProvider>` (see `buildSpatialNavFocusCommand`). Its
  // execute closure invokes `actions.focus(fq)`, which calls
  // `invoke("spatial_focus", { fq, snapshot })`. So the assertion target
  // is the `spatial_focus` IPC call's `fq` payload.
  // -------------------------------------------------------------------------

  it("filter_button_click_dispatches_nav_focus_with_filter_editor_fq", async () => {
    mockResolvedCommands([focusFilterRegistryEntry()]);

    renderTabBar();
    await flushEffects();

    const button = screen.getByRole("button", { name: "Focus Filter" });

    await act(async () => {
      fireEvent.click(button);
      // The click handler dispatches nav.focus, which is a frontend
      // execute (synchronous). The `actions.focus` call then issues
      // `invoke("spatial_focus", ...)` asynchronously. One microtask
      // turn is enough for the synchronous portion to flush.
      await Promise.resolve();
      await Promise.resolve();
    });

    // Expect at least one `spatial_focus` IPC call whose `fq` ends with
    // the filter editor's segment for the active perspective.
    const spatialFocusCalls = mockInvoke.mock.calls.filter(
      (c) => (c[0] === "spatial_focus" || (c[0] === "command_tool_call" && (c[1] as any)?.tool === "focus" && (c[1] as any)?.op === "set focus")),
    );
    expect(
      spatialFocusCalls.length,
      "nav.focus must result in at least one spatial_focus IPC",
    ).toBeGreaterThan(0);

    const lastCall = spatialFocusCalls[spatialFocusCalls.length - 1];
    const fq = (lastCall[1] as { fq?: string })?.fq ?? "";
    expect(
      fq.endsWith("filter_editor:p1"),
      `spatial_focus.fq must end with filter_editor:p1 (got ${fq})`,
    ).toBe(true);

    // Negative: there must be NO backend dispatch_command call for
    // `perspective.filter.focus` (the old parallel-channel path) — the
    // click goes through `nav.focus` only.
    const filterFocusBackendCalls = mockInvoke.mock.calls.filter(
      (c) =>
        c[0] === "dispatch_command" &&
        (c[1] as { cmd?: string })?.cmd === "perspective.filter.focus",
    );
    expect(
      filterFocusBackendCalls,
      "click must not dispatch the deleted perspective.filter.focus backend command",
    ).toHaveLength(0);
  });

  // -------------------------------------------------------------------------
  // 2. Switching the active perspective after render and clicking Filter
  //    lands focus on the new active perspective's filter_editor moniker.
  //
  // Catches an id-resolution regression: a stale captured perspective id
  // would route focus to the wrong scope.
  // -------------------------------------------------------------------------

  it("filter_button_click_targets_the_currently_active_perspective", async () => {
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [
        { id: "p1", name: "Sprint", view: "board" },
        { id: "p2", name: "Triage", view: "board" },
      ],
      activePerspective: { id: "p2", name: "Triage", view: "board" },
    };
    mockResolvedCommands([focusFilterRegistryEntry()]);

    renderTabBar();
    await flushEffects();

    const button = screen.getByRole("button", { name: "Focus Filter" });

    await act(async () => {
      fireEvent.click(button);
      await Promise.resolve();
      await Promise.resolve();
    });

    const spatialFocusCalls = mockInvoke.mock.calls.filter(
      (c) => (c[0] === "spatial_focus" || (c[0] === "command_tool_call" && (c[1] as any)?.tool === "focus" && (c[1] as any)?.op === "set focus")),
    );
    expect(spatialFocusCalls.length).toBeGreaterThan(0);
    const lastCall = spatialFocusCalls[spatialFocusCalls.length - 1];
    const fq = (lastCall[1] as { fq?: string })?.fq ?? "";
    expect(
      fq.endsWith("filter_editor:p2"),
      `spatial_focus.fq must end with filter_editor:p2 when p2 is active (got ${fq})`,
    ).toBe(true);
  });

  // -------------------------------------------------------------------------
  // 3. The Filter `<CommandButton>` is highlighted (`text-primary`) when
  //    the active perspective has a non-empty filter.
  // -------------------------------------------------------------------------

  it("filter_button_is_active_when_perspective_has_a_filter", async () => {
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [
        { id: "p1", name: "Sprint", view: "board", filter: "#bug" },
      ],
      activePerspective: {
        id: "p1",
        name: "Sprint",
        view: "board",
        filter: "#bug",
      },
    };
    mockResolvedCommands([focusFilterRegistryEntry()]);

    renderTabBar();
    await flushEffects();

    const button = screen.getByRole("button", { name: "Focus Filter" });
    // `<CommandButton>` applies `text-primary` whenever `isActive` is
    // true. The migration wires `isActive={Boolean(perspective.filter)}`
    // through `isCommandActiveForPerspective`, so a non-empty filter
    // must light up the icon.
    expect(button.className).toMatch(/text-primary/);
    const svg = button.querySelector("svg");
    expect(svg?.getAttribute("fill")).toBe("currentColor");
  });

  // -------------------------------------------------------------------------
  // 4. The Filter `<CommandButton>` is NOT highlighted when the active
  //    perspective has no filter.
  // -------------------------------------------------------------------------

  it("filter_button_is_inactive_when_perspective_filter_is_undefined", async () => {
    // Default perspective has no `filter` — the beforeEach sets it to
    // `{ id: "p1", name: "Sprint", view: "board" }` with no filter
    // field, so this test relies on the default fixture.
    mockResolvedCommands([focusFilterRegistryEntry()]);

    renderTabBar();
    await flushEffects();

    const button = screen.getByRole("button", { name: "Focus Filter" });
    // Counterpoint to test #2 — the icon must NOT carry the primary
    // highlight when the filter is unset, and the SVG's `fill` must be
    // `none` so the icon renders as a muted glyph.
    expect(button.className).not.toMatch(/text-primary/);
    const svg = button.querySelector("svg");
    expect(svg?.getAttribute("fill")).toBe("none");
  });

  // -------------------------------------------------------------------------
  // 5. The new button uses the registry-derived moniker, NOT the legacy
  //    `perspective_tab.filter:{id}`.
  // -------------------------------------------------------------------------

  it("filter_button_uses_perspective_filter_focus_moniker", async () => {
    mockResolvedCommands([focusFilterRegistryEntry()]);

    const { container } = renderTabBar();
    await flushEffects();

    // The `<CommandButton>` registers its leaf moniker as
    // `${surface}.${command.id}:${surfaceId}` — for the perspective
    // tab bar that's `perspective_tab.perspective.filter.focus:p1`.
    // The DOM mirrors this via `data-segment` on the `<FocusScope>`
    // wrapper Pressable mounts internally.
    const newLeaf = container.querySelector(
      "[data-segment='perspective_tab.perspective.filter.focus:p1']",
    );
    expect(
      newLeaf,
      "the registry-driven `<CommandButton>` must register the new spatial-nav moniker",
    ).not.toBeNull();

    // The legacy moniker (from the deleted `<FilterFocusButton>`)
    // MUST be gone — if a future regression resurrects the hardcoded
    // button this assertion fires.
    const legacyLeaf = container.querySelector(
      "[data-segment='perspective_tab.filter:p1']",
    );
    expect(
      legacyLeaf,
      "the legacy perspective_tab.filter:{id} moniker must be deleted with <FilterFocusButton>",
    ).toBeNull();
  });
});
