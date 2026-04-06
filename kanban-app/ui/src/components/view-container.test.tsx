import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { useContext } from "react";
import { CommandScopeContext, scopeChainFromScope } from "@/lib/command-scope";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import type { ViewDef, BoardData, Entity } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Mocks — Tauri APIs must be mocked before importing components.
// ---------------------------------------------------------------------------

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve([])),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));

// ---------------------------------------------------------------------------
// Mock views-context — ViewContainer reads activeView from it.
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Mock window-container hooks — ViewContainer reads board data from these.
// ---------------------------------------------------------------------------

const mockBoardData = vi.hoisted(() => vi.fn<[], BoardData | null>(() => null));
const mockActiveBoardPath = vi.hoisted(() =>
  vi.fn<[], string | undefined>(() => undefined),
);
const mockEntitiesByType = vi.hoisted(() =>
  vi.fn<[], Record<string, Entity[]>>(() => ({})),
);

vi.mock("@/components/window-container", () => ({
  useBoardData: () => mockBoardData(),
  useActiveBoardPath: () => mockActiveBoardPath(),
  useWindowLoading: () => false,
  useOpenBoards: () => [],
  useHandleSwitchBoard: () => vi.fn(),
}));

vi.mock("@/components/rust-engine-container", () => ({
  RustEngineContainer: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
  useEntitiesByType: () => mockEntitiesByType(),
}));

// Mock ui-state-context for transitive dependencies.
vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => ({ windows: {} }),
}));

// Mock view components — we verify which one renders, not their internals.
vi.mock("@/components/board-view", () => ({
  BoardView: () => <div data-testid="board-view">BoardView</div>,
}));

vi.mock("@/components/grid-view", () => ({
  GridView: () => <div data-testid="grid-view">GridView</div>,
}));

// Import after mocks
import { ViewContainer } from "./view-container";

// ---------------------------------------------------------------------------
// Test data
// ---------------------------------------------------------------------------

const BOARD_VIEW: ViewDef = {
  id: "board-default",
  name: "Board",
  kind: "board",
  icon: "kanban",
};

const GRID_VIEW: ViewDef = {
  id: "grid-default",
  name: "Grid",
  kind: "grid",
  icon: "table",
};

const UNKNOWN_VIEW: ViewDef = {
  id: "custom-1",
  name: "Custom",
  kind: "timeline",
  icon: "clock",
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
// Probes
// ---------------------------------------------------------------------------

/** Reads scope chain and renders it. */
function ScopeChainProbe() {
  const scope = useContext(CommandScopeContext);
  const chain = scopeChainFromScope(scope);
  return <span data-testid="scope-chain">{chain.join(" > ")}</span>;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("ViewContainer", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockViews.mockReturnValue({
      views: [],
      activeView: null,
      setActiveViewId: vi.fn(),
      refresh: vi.fn(),
    });
    mockBoardData.mockReturnValue(MOCK_BOARD);
    mockActiveBoardPath.mockReturnValue("/path/to/board");
    mockEntitiesByType.mockReturnValue({});
  });

  it("renders BoardView when activeView is null (default)", () => {
    render(
      <EntityFocusProvider>
        <ViewContainer />
      </EntityFocusProvider>,
    );
    expect(screen.getByTestId("board-view")).toBeTruthy();
  });

  it("renders BoardView when activeView.kind is 'board'", () => {
    mockViews.mockReturnValue({
      views: [BOARD_VIEW],
      activeView: BOARD_VIEW,
      setActiveViewId: vi.fn(),
      refresh: vi.fn(),
    });

    render(
      <EntityFocusProvider>
        <ViewContainer />
      </EntityFocusProvider>,
    );
    expect(screen.getByTestId("board-view")).toBeTruthy();
  });

  it("renders GridView when activeView.kind is 'grid'", () => {
    mockViews.mockReturnValue({
      views: [GRID_VIEW],
      activeView: GRID_VIEW,
      setActiveViewId: vi.fn(),
      refresh: vi.fn(),
    });

    render(
      <EntityFocusProvider>
        <ViewContainer />
      </EntityFocusProvider>,
    );
    expect(screen.getByTestId("grid-view")).toBeTruthy();
  });

  it("renders placeholder for unknown view kind", () => {
    mockViews.mockReturnValue({
      views: [UNKNOWN_VIEW],
      activeView: UNKNOWN_VIEW,
      setActiveViewId: vi.fn(),
      refresh: vi.fn(),
    });

    render(
      <EntityFocusProvider>
        <ViewContainer />
      </EntityFocusProvider>,
    );
    expect(screen.getByText(/not yet implemented/)).toBeTruthy();
    expect(screen.getByText(/Custom/)).toBeTruthy();
  });

  it("provides CommandScopeProvider with view:{id} moniker", () => {
    mockViews.mockReturnValue({
      views: [GRID_VIEW],
      activeView: GRID_VIEW,
      setActiveViewId: vi.fn(),
      refresh: vi.fn(),
    });

    render(
      <EntityFocusProvider>
        <ViewContainer>
          <ScopeChainProbe />
        </ViewContainer>
      </EntityFocusProvider>,
    );

    const chain = screen.getByTestId("scope-chain").textContent!;
    expect(chain).toContain("view:grid-default");
  });
});
