import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { TooltipProvider } from "@/components/ui/tooltip";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
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

// EntityFocusProvider (mounted around NavBar for the FocusScope wrappers)
// resolves the current webview via getCurrentWebviewWindow; stub it
// with a minimal implementation rather than letting the real Tauri
// module try to reach a runtime bridge that doesn't exist in the
// headless browser.
vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({
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
// Mock WindowContainer hooks — NavBar reads these from context.
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

// ---------------------------------------------------------------------------
// Mock command-scope — NavBar uses useDispatchCommand for inspect/search.
// ---------------------------------------------------------------------------

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

/**
 * Renders NavBar inside the required providers.
 *
 * `TooltipProvider` is needed for the Inspect/Search tooltips; the
 * `EntityFocusProvider` is needed by each toolbar element's `FocusScope`
 * wrapper (registers/unregisters focus claims). Both are mounted high
 * in the production component tree; tests reproduce that here.
 */
function renderNavBar() {
  return render(
    <EntityFocusProvider>
      <TooltipProvider>
        <NavBar />
      </TooltipProvider>
    </EntityFocusProvider>,
  );
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

describe("NavBar", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockBoardData.mockReturnValue(null);
    mockOpenBoards.mockReturnValue([]);
    mockActiveBoardPath.mockReturnValue(undefined);
  });

  it("renders without props", () => {
    renderNavBar();
    // Should render the search button at minimum
    expect(screen.getByLabelText("Search")).toBeTruthy();
  });

  it("renders board selector with open boards from context", () => {
    mockOpenBoards.mockReturnValue(MOCK_OPEN_BOARDS);
    mockActiveBoardPath.mockReturnValue("/boards/a/.kanban");

    renderNavBar();
    // BoardSelector renders board names -- the active one should be visible
    expect(screen.getByText("Board A")).toBeTruthy();
  });

  it("renders inspect button when board is loaded", () => {
    mockBoardData.mockReturnValue(MOCK_BOARD);

    renderNavBar();
    expect(screen.getByLabelText("Inspect board")).toBeTruthy();
  });

  it("does not render inspect button when no board is loaded", () => {
    mockBoardData.mockReturnValue(null);

    renderNavBar();
    expect(screen.queryByLabelText("Inspect board")).toBeNull();
  });

  it("dispatches ui.inspect on inspect button click", () => {
    mockBoardData.mockReturnValue(MOCK_BOARD);

    renderNavBar();
    fireEvent.click(screen.getByLabelText("Inspect board"));
    expect(mockDispatchInspect).toHaveBeenCalled();
  });

  it("dispatches app.search on search button click", () => {
    renderNavBar();
    fireEvent.click(screen.getByLabelText("Search"));
    expect(mockDispatchSearch).toHaveBeenCalled();
  });

  it("renders percent complete field when board is loaded", () => {
    mockBoardData.mockReturnValue(MOCK_BOARD);

    renderNavBar();
    expect(screen.getByTestId("field-percent")).toBeTruthy();
    expect(screen.getByTestId("field-percent").textContent).toBe("b1");
  });

  it("does not render percent complete field when no board", () => {
    mockBoardData.mockReturnValue(null);

    renderNavBar();
    expect(screen.queryByTestId("field-percent")).toBeNull();
  });

  it("renders progress bar when isBusy is true", () => {
    mockIsBusy.mockReturnValue(true);

    renderNavBar();
    expect(screen.getByRole("progressbar")).toBeTruthy();
  });

  it("does not render progress bar when isBusy is false", () => {
    mockIsBusy.mockReturnValue(false);

    renderNavBar();
    expect(screen.queryByRole("progressbar")).toBeNull();
  });
});

/**
 * Spatial-nav contract: every interactive element in NavBar must carry a
 * `data-moniker` (with `toolbar:` prefix) so the Rust spatial engine has
 * a rect to register and `k`/`j` from adjacent strips can land on the
 * toolbar. Without these attributes, `FocusScope.useSpatialClaim` never
 * registers a rect and the toolbar is spatially invisible.
 */
describe("NavBar spatial monikers", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockBoardData.mockReturnValue(MOCK_BOARD);
    mockOpenBoards.mockReturnValue(MOCK_OPEN_BOARDS);
    mockActiveBoardPath.mockReturnValue("/boards/a/.kanban");
  });

  it("board selector exposes toolbar:board-selector moniker", () => {
    renderNavBar();
    expect(
      screen.getByTestId("data-moniker:toolbar:board-selector"),
    ).toBeTruthy();
  });

  it("inspect button exposes toolbar:inspect-board moniker when board is loaded", () => {
    renderNavBar();
    expect(
      screen.getByTestId("data-moniker:toolbar:inspect-board"),
    ).toBeTruthy();
  });

  it("percent-complete field exposes toolbar:percent-complete moniker when board is loaded", () => {
    renderNavBar();
    expect(
      screen.getByTestId("data-moniker:toolbar:percent-complete"),
    ).toBeTruthy();
  });

  it("search button exposes toolbar:search moniker", () => {
    renderNavBar();
    expect(screen.getByTestId("data-moniker:toolbar:search")).toBeTruthy();
  });
});
