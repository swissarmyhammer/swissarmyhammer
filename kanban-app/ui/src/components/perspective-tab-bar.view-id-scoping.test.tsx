/**
 * Regression tests pinning the view_id-first / kind-fallback filter rule
 * used by `usePerspectiveTabBar` to decide which perspective tabs to show.
 *
 * Source-of-truth for task `01KRC1F2D259GQDN83M1YVPX0R` (#perspective-view-id):
 *
 *   - When a perspective carries `view_id`, the tab bar shows it only for
 *     the view whose `id` matches that `view_id`. Two sibling views of the
 *     same kind never share view_id-scoped perspectives.
 *   - When a perspective omits `view_id` (legacy shared-by-kind), the tab
 *     bar shows it for every view whose `kind` matches `view`. The legacy
 *     compatibility rule documented on `PerspectiveDef` in
 *     `kanban-app/ui/src/types/kanban.ts` is honored on the client.
 *
 * Mirrors `perspective-tab-bar.test.tsx`'s mock harness (perspectives +
 * views + UI state + schema + entity store + window-container). We do NOT
 * mock `@/lib/command-scope` so the production dispatch path runs end to
 * end — that lets us assert the "+" button's `perspective.save` dispatch
 * actually carries `view_id` for the active view.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { TooltipProvider } from "@/components/ui/tooltip";

// Mock Tauri APIs before importing any modules that use them.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((..._args: any[]) => Promise.resolve(null));
vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
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

// Track perspective context values for assertions.
const mockSetActivePerspectiveId = vi.fn();
const mockRefresh = vi.fn(() => Promise.resolve());

/** Shape of a perspective in the mock context — mirrors PerspectiveDef. */
type MockPerspective = {
  id: string;
  name: string;
  view: string;
  view_id?: string;
  filter?: string;
  group?: string;
};

/** Shape of a view in the mock views context — mirrors ViewDef. */
type MockView = {
  id: string;
  name: string;
  kind: string;
  icon?: string;
};

// Mock the perspectives context so we can control perspectives list.
let mockPerspectivesValue = {
  perspectives: [] as MockPerspective[],
  activePerspective: null as MockPerspective | null,
  setActivePerspectiveId: mockSetActivePerspectiveId,
  refresh: mockRefresh,
};

vi.mock("@/lib/perspective-context", () => ({
  usePerspectives: () => mockPerspectivesValue,
}));

// Mock the views context so we can swap between view-a / view-b / board.
const VIEW_A: MockView = { id: "view-a", name: "Grid A", kind: "grid" };
const VIEW_B: MockView = { id: "view-b", name: "Grid B", kind: "grid" };
const VIEW_BOARD: MockView = { id: "board-1", name: "Board", kind: "board" };

let mockViewsValue = {
  views: [VIEW_A, VIEW_B, VIEW_BOARD],
  activeView: VIEW_A as MockView | null,
  setActiveViewId: vi.fn(),
  refresh: vi.fn(() => Promise.resolve()),
};

vi.mock("@/lib/views-context", () => ({
  useViews: () => mockViewsValue,
}));

// Mock useContextMenu — returns a handler that records calls.
vi.mock("@/lib/context-menu", () => ({
  useContextMenu: () => vi.fn(),
}));

// Mock useEntityStore — needed by useMentionExtensions (used in FilterEditor).
vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({ getEntities: () => [] }),
}));

// Mock board data context — provides virtual tag metadata from the backend.
vi.mock("@/components/window-container", () => ({
  useBoardData: () => ({ virtualTagMeta: [] }),
}));

// Mock useSchema — returns empty schema by default.
vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => ({ entity: { name: "task", fields: [] }, fields: [] }),
    getFieldDef: () => undefined,
    mentionableTypes: [],
    loading: false,
  }),
}));

// Mock useUIState — required by TextEditor (CM6 keymap selection).
const mockUIState = () => ({
  keymap_mode: "cua",
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

import { PerspectiveTabBar } from "./perspective-tab-bar";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { FocusLayer } from "./focus-layer";
import { asSegment } from "@/types/spatial";

/** Render the bar inside the standard provider stack used by sibling tests. */
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
 * Fixture: two grid views (view-a, view-b) + one board view.
 *
 *   - P1: pinned to view-a via `view_id: "view-a"` (kind grid).
 *   - P2: pinned to view-b via `view_id: "view-b"` (kind grid).
 *   - P3: legacy grid-shared perspective with no `view_id` — appears in
 *         every kind-grid view, NOT in the board view.
 *   - P4: legacy board-shared perspective with no `view_id` — appears in
 *         the board view only (sanity check that kind-fallback respects
 *         kind boundaries, doesn't leak across kinds).
 */
const P1: MockPerspective = {
  id: "p1",
  name: "Grid A View",
  view: "grid",
  view_id: "view-a",
};
const P2: MockPerspective = {
  id: "p2",
  name: "Grid B View",
  view: "grid",
  view_id: "view-b",
};
const P3: MockPerspective = {
  id: "p3",
  name: "Legacy Grid",
  view: "grid",
};
const P4: MockPerspective = {
  id: "p4",
  name: "Board Only",
  view: "board",
};

describe("PerspectiveTabBar — view_id-first / kind-fallback scoping", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockPerspectivesValue = {
      perspectives: [P1, P2, P3, P4],
      activePerspective: null,
      setActivePerspectiveId: mockSetActivePerspectiveId,
      refresh: mockRefresh,
    };
    mockViewsValue = {
      views: [VIEW_A, VIEW_B, VIEW_BOARD],
      activeView: VIEW_A,
      setActiveViewId: vi.fn(),
      refresh: vi.fn(() => Promise.resolve()),
    };
  });

  it("active view-a shows P1 (view_id match) but not P2 (view_id sibling)", () => {
    mockViewsValue = { ...mockViewsValue, activeView: VIEW_A };
    renderTabBar();

    expect(screen.getByText("Grid A View")).toBeDefined();
    expect(screen.queryByText("Grid B View")).toBeNull();
  });

  it("active view-b shows P2 (view_id match) but not P1 (view_id sibling)", () => {
    mockViewsValue = { ...mockViewsValue, activeView: VIEW_B };
    renderTabBar();

    expect(screen.getByText("Grid B View")).toBeDefined();
    expect(screen.queryByText("Grid A View")).toBeNull();
  });

  it("legacy P3 (no view_id) appears in both grid views via kind fallback", () => {
    // view-a
    mockViewsValue = { ...mockViewsValue, activeView: VIEW_A };
    const a = renderTabBar();
    expect(screen.getByText("Legacy Grid")).toBeDefined();
    a.unmount();

    // view-b
    mockViewsValue = { ...mockViewsValue, activeView: VIEW_B };
    renderTabBar();
    expect(screen.getByText("Legacy Grid")).toBeDefined();
  });

  it("legacy P3 (kind grid) does NOT appear in the board view", () => {
    mockViewsValue = { ...mockViewsValue, activeView: VIEW_BOARD };
    renderTabBar();

    expect(screen.queryByText("Legacy Grid")).toBeNull();
    // Board-kind legacy perspective is still visible — kind fallback respects kind.
    expect(screen.getByText("Board Only")).toBeDefined();
  });

  it("view_id-scoped P1 does NOT appear in the board view", () => {
    mockViewsValue = { ...mockViewsValue, activeView: VIEW_BOARD };
    renderTabBar();

    expect(screen.queryByText("Grid A View")).toBeNull();
    expect(screen.queryByText("Grid B View")).toBeNull();
  });

  it("clicking '+' on view-a dispatches perspective.save with view_id: view-a", () => {
    mockViewsValue = { ...mockViewsValue, activeView: VIEW_A };
    renderTabBar();

    const addButton = screen.getByRole("button", { name: /add perspective/i });
    fireEvent.click(addButton);

    expect(mockInvoke).toHaveBeenCalledWith(
      "dispatch_command",
      expect.objectContaining({
        cmd: "perspective.save",
        args: expect.objectContaining({
          name: expect.any(String),
          view: "grid",
          view_id: "view-a",
        }),
      }),
    );
  });

  it("clicking '+' on view-b dispatches perspective.save with view_id: view-b", () => {
    mockViewsValue = { ...mockViewsValue, activeView: VIEW_B };
    renderTabBar();

    const addButton = screen.getByRole("button", { name: /add perspective/i });
    fireEvent.click(addButton);

    expect(mockInvoke).toHaveBeenCalledWith(
      "dispatch_command",
      expect.objectContaining({
        cmd: "perspective.save",
        args: expect.objectContaining({
          name: expect.any(String),
          view: "grid",
          view_id: "view-b",
        }),
      }),
    );
  });
});
