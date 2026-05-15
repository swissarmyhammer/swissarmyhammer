import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { useContext } from "react";
import { CommandScopeContext } from "@/lib/command-scope";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import type { ViewDef } from "@/types/kanban";

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
// Mock views-context — ViewsContainer wraps ViewsProvider, but we control
// what useViews returns so we can verify command generation.
// ---------------------------------------------------------------------------

const MOCK_VIEWS: ViewDef[] = [
  { id: "board-default", name: "Board", kind: "board", icon: "kanban" },
  { id: "grid-default", name: "Grid", kind: "grid", icon: "table" },
];

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

// Mock LeftNav — we only care that it renders, not its internals.
vi.mock("@/components/left-nav", () => ({
  LeftNav: () => <nav data-testid="left-nav">LeftNav</nav>,
}));

// Mock ui-state-context for any transitive dependencies.
vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => ({ windows: {} }),
}));

// Import after mocks
import { ViewsContainer } from "./views-container";

// ---------------------------------------------------------------------------
// Probes
// ---------------------------------------------------------------------------

/** Reads registered commands from the scope and renders their IDs. */
function CommandProbe() {
  const scope = useContext(CommandScopeContext);
  const ids = scope ? Array.from(scope.commands.keys()).sort() : [];
  return <span data-testid="commands">{ids.join(",")}</span>;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("ViewsContainer", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockViews.mockReturnValue({
      views: [],
      activeView: null,
      setActiveViewId: vi.fn(),
      refresh: vi.fn(),
    });
  });

  it("renders children", () => {
    render(
      <EntityFocusProvider>
        <ViewsContainer>
          <span data-testid="child">hello</span>
        </ViewsContainer>
      </EntityFocusProvider>,
    );
    expect(screen.getByTestId("child").textContent).toBe("hello");
  });

  it("renders LeftNav as sidebar", () => {
    render(
      <EntityFocusProvider>
        <ViewsContainer>
          <span>content</span>
        </ViewsContainer>
      </EntityFocusProvider>,
    );
    expect(screen.getByTestId("left-nav")).toBeTruthy();
  });

  it("registers view.switch commands from the views list", () => {
    mockViews.mockReturnValue({
      views: MOCK_VIEWS,
      activeView: MOCK_VIEWS[0],
      setActiveViewId: vi.fn(),
      refresh: vi.fn(),
    });

    render(
      <EntityFocusProvider>
        <ViewsContainer>
          <CommandProbe />
        </ViewsContainer>
      </EntityFocusProvider>,
    );

    const cmds = screen.getByTestId("commands").textContent!;
    expect(cmds).toContain("view.switch:board-default");
    expect(cmds).toContain("view.switch:grid-default");
  });

  it("registers no commands when views list is empty", () => {
    render(
      <EntityFocusProvider>
        <ViewsContainer>
          <CommandProbe />
        </ViewsContainer>
      </EntityFocusProvider>,
    );

    expect(screen.getByTestId("commands").textContent).toBe("");
  });
});
