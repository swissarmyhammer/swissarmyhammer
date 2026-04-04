import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import type { BoardData, OpenBoard } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Mock Tauri APIs before importing components.
// ---------------------------------------------------------------------------

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
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
// Mock WindowContainer hooks — NavBar reads these from context.
// ---------------------------------------------------------------------------

const mockBoardData = vi.hoisted(() => vi.fn<[], BoardData | null>(() => null));
const mockOpenBoards = vi.hoisted(() => vi.fn<[], OpenBoard[]>(() => []));
const mockActiveBoardPath = vi.hoisted(() =>
  vi.fn<[], string | undefined>(() => undefined),
);
const mockHandleSwitchBoard = vi.hoisted(() => vi.fn<[string], void>());

vi.mock("@/components/window-container", () => ({
  useBoardData: () => mockBoardData(),
  useOpenBoards: () => mockOpenBoards(),
  useActiveBoardPath: () => mockActiveBoardPath(),
  useHandleSwitchBoard: () => mockHandleSwitchBoard,
}));

// ---------------------------------------------------------------------------
// Mock command-scope — NavBar uses useDispatchCommand for inspect/search.
// ---------------------------------------------------------------------------

const mockDispatchInspect = vi.hoisted(() => vi.fn(() => Promise.resolve()));
const mockDispatchSearch = vi.hoisted(() => vi.fn(() => Promise.resolve()));

vi.mock("@/lib/command-scope", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/command-scope")>();
  return {
    ...actual,
    useDispatchCommand: (cmd: string) => {
      if (cmd === "ui.inspect") return mockDispatchInspect;
      if (cmd === "app.search") return mockDispatchSearch;
      return vi.fn(() => Promise.resolve());
    },
  };
});

// ---------------------------------------------------------------------------
// Mock schema-context — NavBar uses useSchema for percent_complete field def.
// ---------------------------------------------------------------------------

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
    getEntityCommands: () => [],
    mentionableTypes: [],
    loading: false,
  }),
}));

// ---------------------------------------------------------------------------
// Mock the Field component to avoid deep rendering.
// ---------------------------------------------------------------------------

vi.mock("@/components/fields/field", () => ({
  Field: (props: Record<string, unknown>) => (
    <span data-testid="field-percent">{String(props.entityId)}</span>
  ),
}));

// Import after mocks
import { NavBar } from "./nav-bar";

// ---------------------------------------------------------------------------
// Test data
// ---------------------------------------------------------------------------

const MOCK_BOARD: BoardData = {
  board: {
    entity_type: "board",
    id: "b1",
    fields: { name: { String: "Test Board" } },
  },
  columns: [],
  swimlanes: [],
  tags: [],
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

describe("NavBar", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockBoardData.mockReturnValue(null);
    mockOpenBoards.mockReturnValue([]);
    mockActiveBoardPath.mockReturnValue(undefined);
  });

  it("renders without props", () => {
    render(<NavBar />);
    // Should render the search button at minimum
    expect(screen.getByTitle("Search")).toBeTruthy();
  });

  it("renders board selector with open boards from context", () => {
    mockOpenBoards.mockReturnValue(MOCK_OPEN_BOARDS);
    mockActiveBoardPath.mockReturnValue("/boards/a/.kanban");

    render(<NavBar />);
    // BoardSelector renders board names -- the active one should be visible
    expect(screen.getByText("Board A")).toBeTruthy();
  });

  it("renders inspect button when board is loaded", () => {
    mockBoardData.mockReturnValue(MOCK_BOARD);

    render(<NavBar />);
    expect(screen.getByTitle("Inspect board")).toBeTruthy();
  });

  it("does not render inspect button when no board is loaded", () => {
    mockBoardData.mockReturnValue(null);

    render(<NavBar />);
    expect(screen.queryByTitle("Inspect board")).toBeNull();
  });

  it("dispatches ui.inspect on inspect button click", () => {
    mockBoardData.mockReturnValue(MOCK_BOARD);

    render(<NavBar />);
    fireEvent.click(screen.getByTitle("Inspect board"));
    expect(mockDispatchInspect).toHaveBeenCalled();
  });

  it("dispatches app.search on search button click", () => {
    render(<NavBar />);
    fireEvent.click(screen.getByTitle("Search"));
    expect(mockDispatchSearch).toHaveBeenCalled();
  });

  it("renders percent complete field when board is loaded", () => {
    mockBoardData.mockReturnValue(MOCK_BOARD);

    render(<NavBar />);
    expect(screen.getByTestId("field-percent")).toBeTruthy();
    expect(screen.getByTestId("field-percent").textContent).toBe("b1");
  });

  it("does not render percent complete field when no board", () => {
    mockBoardData.mockReturnValue(null);

    render(<NavBar />);
    expect(screen.queryByTestId("field-percent")).toBeNull();
  });
});
