import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { useContext } from "react";
import { CommandScopeContext, scopeChainFromScope } from "@/lib/command-scope";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import type { BoardData } from "@/types/kanban";

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
// Mock WindowContainer hooks — BoardContainer reads these.
// ---------------------------------------------------------------------------

const mockBoardData = vi.hoisted(() => vi.fn<() => BoardData | null>(() => null));
const mockLoading = vi.hoisted(() => vi.fn<() => boolean>(() => false));
const mockActiveBoardPath = vi.hoisted(() =>
  vi.fn<() => string | undefined>(() => undefined),
);

vi.mock("@/components/window-container", () => ({
  useBoardData: () => mockBoardData(),
  useWindowLoading: () => mockLoading(),
  useActiveBoardPath: () => mockActiveBoardPath(),
}));

// Import after mocks
import { BoardContainer, useBoardContext } from "./board-container";

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
// Probe components
// ---------------------------------------------------------------------------

/** Reads the scope chain and renders it. */
function ScopeChainProbe() {
  const scope = useContext(CommandScopeContext);
  const chain = scopeChainFromScope(scope);
  return <span data-testid="scope-chain">{chain.join(" > ")}</span>;
}

/** Reads board context and renders board id. */
function BoardContextProbe() {
  const ctx = useBoardContext();
  return (
    <span data-testid="board-context">
      {ctx.board ? ctx.board.board.id : "none"}
    </span>
  );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("BoardContainer", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockBoardData.mockReturnValue(null);
    mockLoading.mockReturnValue(false);
    mockActiveBoardPath.mockReturnValue(undefined);
  });

  it("renders loading spinner when loading is true", () => {
    mockLoading.mockReturnValue(true);

    render(
      <EntityFocusProvider>
        <BoardContainer>
          <span data-testid="child">should not show</span>
        </BoardContainer>
      </EntityFocusProvider>,
    );

    // Spinner should be visible
    expect(screen.getByRole("status")).toBeTruthy();
    // Children should NOT be visible
    expect(screen.queryByTestId("child")).toBeNull();
  });

  it("loading spinner container uses h-screen for viewport centering", () => {
    mockLoading.mockReturnValue(true);

    render(
      <EntityFocusProvider>
        <BoardContainer>
          <span>child</span>
        </BoardContainer>
      </EntityFocusProvider>,
    );

    const status = screen.getByRole("status");
    expect(status.className).toContain("h-screen");
    expect(status.className).not.toContain("flex-1");
  });

  it("no-board placeholder uses h-screen for viewport centering", () => {
    mockBoardData.mockReturnValue(null);
    mockLoading.mockReturnValue(false);

    render(
      <EntityFocusProvider>
        <BoardContainer>
          <span>child</span>
        </BoardContainer>
      </EntityFocusProvider>,
    );

    const main = screen.getByText("No board loaded").closest("main")!;
    expect(main.className).toContain("h-screen");
    expect(main.className).not.toContain("flex-1");
  });

  it("renders placeholder when no board is loaded and not loading", () => {
    mockBoardData.mockReturnValue(null);
    mockLoading.mockReturnValue(false);

    render(
      <EntityFocusProvider>
        <BoardContainer>
          <span data-testid="child">should not show</span>
        </BoardContainer>
      </EntityFocusProvider>,
    );

    expect(screen.getByText("No board loaded")).toBeTruthy();
    expect(screen.queryByTestId("child")).toBeNull();
  });

  it("renders children when board is loaded", () => {
    mockBoardData.mockReturnValue(MOCK_BOARD);
    mockActiveBoardPath.mockReturnValue("/path/to/board");

    render(
      <EntityFocusProvider>
        <BoardContainer>
          <span data-testid="child">board content</span>
        </BoardContainer>
      </EntityFocusProvider>,
    );

    expect(screen.getByTestId("child").textContent).toBe("board content");
  });

  it("provides BoardContext with board data to descendants", () => {
    mockBoardData.mockReturnValue(MOCK_BOARD);
    mockActiveBoardPath.mockReturnValue("/path/to/board");

    render(
      <EntityFocusProvider>
        <BoardContainer>
          <BoardContextProbe />
        </BoardContainer>
      </EntityFocusProvider>,
    );

    expect(screen.getByTestId("board-context").textContent).toBe("b1");
  });

  it("BoardContext is null when no board is loaded", () => {
    mockBoardData.mockReturnValue(null);

    render(
      <EntityFocusProvider>
        <BoardContainer>
          <BoardContextProbe />
        </BoardContainer>
      </EntityFocusProvider>,
    );

    // Children aren't rendered when no board, so probe won't be in DOM
    expect(screen.queryByTestId("board-context")).toBeNull();
  });

  it("has board:{boardId} in the scope chain when board is active", () => {
    mockBoardData.mockReturnValue(MOCK_BOARD);
    mockActiveBoardPath.mockReturnValue("/path/to/board");

    render(
      <EntityFocusProvider>
        <BoardContainer>
          <ScopeChainProbe />
        </BoardContainer>
      </EntityFocusProvider>,
    );

    const chain = screen.getByTestId("scope-chain").textContent!;
    expect(chain).toContain("board:b1");
  });
});
