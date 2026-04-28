import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { BoardData, OpenBoard } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Mock Tauri APIs before importing components.
// ---------------------------------------------------------------------------

const mockInvoke = vi.hoisted(() =>
  vi.fn((..._args: unknown[]) => Promise.resolve()),
);

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
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
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { asLayerName } from "@/types/spatial";

/** Identity-stable layer name for the test window root, matches App.tsx. */
const WINDOW_LAYER_NAME = asLayerName("window");

/**
 * Render `NavBar` inside the spatial-focus + window-root layer providers
 * that the production tree mounts in `App.tsx`.
 *
 * `NavBar` is wrapped in a `<FocusZone>`, which registers via
 * `spatial_register_zone` only inside a `<FocusLayer>` — production wraps
 * everything in one, so we mirror that here to exercise the spatial-context
 * path.
 */
function renderNavBar() {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={WINDOW_LAYER_NAME}>
        <TooltipProvider>
          <NavBar />
        </TooltipProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

/** Flush microtasks queued by the spatial-focus register effects. */
async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
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
    mockIsBusy.mockReturnValue(false);
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

  // -------------------------------------------------------------------------
  // Spatial-nav wiring
  //
  // The nav bar mounts as `<FocusZone moniker="ui:navbar">` with each
  // actionable child registered as a `<FocusScope>` leaf whose `parent_zone`
  // is the navbar zone. These tests assert the structural wiring — the
  // spatial-graph contract the rest of the app relies on for arrow nav.
  // -------------------------------------------------------------------------

  /** Filter mock invoke calls to those whose first arg matches `cmd`. */
  function callsFor(cmd: string): Record<string, unknown>[] {
    return mockInvoke.mock.calls
      .filter((c) => c[0] === cmd)
      .map((c) => c[1] as Record<string, unknown>);
  }

  it("exposes the implicit banner landmark for screen readers", () => {
    // Replacing the previous <header> with <FocusZone> (a <div>) used to drop
    // the implicit `role="banner"` landmark — losing the top-of-page anchor
    // that screen-reader users navigate to. The FocusZone now forwards
    // `role="banner"`; this test guards against that regression.
    renderNavBar();
    expect(screen.getByRole("banner")).toBeTruthy();
  });

  it("registers as a FocusZone with moniker ui:navbar at the layer root", async () => {
    renderNavBar();
    await flushSetup();

    const zoneCalls = callsFor("spatial_register_zone");
    const navbarZone = zoneCalls.find((c) => c.moniker === "ui:navbar");
    expect(navbarZone).toBeDefined();
    expect(navbarZone!.parentZone).toBeNull();
    expect(navbarZone!.layerKey).toBeTruthy();
  });

  it("registers ui:navbar.board-selector as a FocusScope child of the navbar zone", async () => {
    mockOpenBoards.mockReturnValue(MOCK_OPEN_BOARDS);
    mockActiveBoardPath.mockReturnValue("/boards/a/.kanban");

    renderNavBar();
    await flushSetup();

    const zoneCalls = callsFor("spatial_register_zone");
    const navbarZone = zoneCalls.find((c) => c.moniker === "ui:navbar");
    expect(navbarZone).toBeDefined();

    const focusableCalls = callsFor("spatial_register_scope");
    const leaf = focusableCalls.find(
      (c) => c.moniker === "ui:navbar.board-selector",
    );
    expect(leaf).toBeDefined();
    expect(leaf!.parentZone).toBe(navbarZone!.key);
  });

  it("registers ui:navbar.inspect as a FocusScope child only when a board is loaded", async () => {
    mockBoardData.mockReturnValue(MOCK_BOARD);

    renderNavBar();
    await flushSetup();

    const zoneCalls = callsFor("spatial_register_zone");
    const navbarZone = zoneCalls.find((c) => c.moniker === "ui:navbar");
    expect(navbarZone).toBeDefined();

    const focusableCalls = callsFor("spatial_register_scope");
    const leaf = focusableCalls.find((c) => c.moniker === "ui:navbar.inspect");
    expect(leaf).toBeDefined();
    expect(leaf!.parentZone).toBe(navbarZone!.key);
  });

  it("does not register ui:navbar.inspect when no board is loaded", async () => {
    mockBoardData.mockReturnValue(null);

    renderNavBar();
    await flushSetup();

    const focusableCalls = callsFor("spatial_register_scope");
    expect(
      focusableCalls.find((c) => c.moniker === "ui:navbar.inspect"),
    ).toBeUndefined();
  });

  it("registers ui:navbar.search as a FocusScope child of the navbar zone", async () => {
    renderNavBar();
    await flushSetup();

    const zoneCalls = callsFor("spatial_register_zone");
    const navbarZone = zoneCalls.find((c) => c.moniker === "ui:navbar");
    expect(navbarZone).toBeDefined();

    const focusableCalls = callsFor("spatial_register_scope");
    const leaf = focusableCalls.find((c) => c.moniker === "ui:navbar.search");
    expect(leaf).toBeDefined();
    expect(leaf!.parentZone).toBe(navbarZone!.key);
  });

  it("regression: does not attach a global keydown listener for legacy nav", () => {
    // The nav bar is purely declarative — arrow-key traversal is handled by
    // the Rust spatial navigator, not by component-level keyboard handlers.
    // Wiring up a `keydown` listener on `document` or `window` would resurrect
    // the legacy nav model and race the spatial graph.
    const docSpy = vi.spyOn(document, "addEventListener");
    const winSpy = vi.spyOn(window, "addEventListener");

    renderNavBar();

    const docKeydown = docSpy.mock.calls.filter((c) => c[0] === "keydown");
    const winKeydown = winSpy.mock.calls.filter((c) => c[0] === "keydown");
    expect(docKeydown).toHaveLength(0);
    expect(winKeydown).toHaveLength(0);

    docSpy.mockRestore();
    winSpy.mockRestore();
  });
});
