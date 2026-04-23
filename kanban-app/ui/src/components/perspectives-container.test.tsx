import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import type { PerspectiveDef } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Mocks — Tauri APIs must be mocked before importing components.
// ---------------------------------------------------------------------------

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve(null)),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));

// ---------------------------------------------------------------------------
// Mock perspective-context — PerspectivesContainer wraps PerspectiveProvider,
// but we control what usePerspectives returns.
// ---------------------------------------------------------------------------

const mockPerspectives: PerspectiveDef[] = [
  { id: "p1", name: "Default", view: "board" },
  { id: "p2", name: "Bugs Only", view: "board", filter: "Status === 'bug'" },
];

const mockUsePerspectives = vi.hoisted(() =>
  vi.fn(() => ({
    perspectives: [] as PerspectiveDef[],
    activePerspective: null as PerspectiveDef | null,
    setActivePerspectiveId: vi.fn(),
    refresh: vi.fn(),
  })),
);

vi.mock("@/lib/perspective-context", () => ({
  PerspectiveProvider: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
  usePerspectives: () => mockUsePerspectives(),
}));

// Mock views-context — PerspectiveTabBar needs it.
vi.mock("@/lib/views-context", () => ({
  useViews: () => ({
    views: [{ id: "board-1", name: "Board", kind: "board", icon: "kanban" }],
    activeView: {
      id: "board-1",
      name: "Board",
      kind: "board",
      icon: "kanban",
    },
    setActiveViewId: vi.fn(),
    refresh: vi.fn(),
  }),
}));

// Mock PerspectiveTabBar — we just verify it renders, not its internals.
vi.mock("@/components/perspective-tab-bar", () => ({
  PerspectiveTabBar: () => (
    <div data-testid="perspective-tab-bar">PerspectiveTabBar</div>
  ),
}));

// Mock ui-state-context for transitive dependencies.
vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => ({ windows: {} }),
}));

// Mock schema-context for transitive dependencies.
vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => null,
    getEntityCommands: () => [],
  }),
}));

// Import after mocks
import { PerspectivesContainer } from "./perspectives-container";
import { usePerspectives } from "@/lib/perspective-context";

// ---------------------------------------------------------------------------
// Probes
// ---------------------------------------------------------------------------

/** Reads the perspectives context and renders the count. */
function PerspectiveProbe() {
  const { perspectives } = usePerspectives();
  return <span data-testid="perspective-count">{perspectives.length}</span>;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("PerspectivesContainer", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUsePerspectives.mockReturnValue({
      perspectives: [],
      activePerspective: null,
      setActivePerspectiveId: vi.fn(),
      refresh: vi.fn(),
    });
  });

  it("renders children", () => {
    render(
      <EntityFocusProvider>
        <PerspectivesContainer>
          <span data-testid="child">hello</span>
        </PerspectivesContainer>
      </EntityFocusProvider>,
    );
    expect(screen.getByTestId("child").textContent).toBe("hello");
  });

  it("renders PerspectiveTabBar", () => {
    render(
      <EntityFocusProvider>
        <PerspectivesContainer>
          <span>content</span>
        </PerspectivesContainer>
      </EntityFocusProvider>,
    );
    expect(screen.getByTestId("perspective-tab-bar")).toBeTruthy();
  });

  it("renders PerspectiveTabBar before children in DOM order", () => {
    render(
      <EntityFocusProvider>
        <PerspectivesContainer>
          <span data-testid="child">content</span>
        </PerspectivesContainer>
      </EntityFocusProvider>,
    );
    const tabBar = screen.getByTestId("perspective-tab-bar");
    const child = screen.getByTestId("child");
    // Tab bar should come before child in document order
    expect(
      tabBar.compareDocumentPosition(child) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it("makes PerspectiveProvider context available to children", () => {
    mockUsePerspectives.mockReturnValue({
      perspectives: mockPerspectives,
      activePerspective: mockPerspectives[0],
      setActivePerspectiveId: vi.fn(),
      refresh: vi.fn(),
    });

    render(
      <EntityFocusProvider>
        <PerspectivesContainer>
          <PerspectiveProbe />
        </PerspectivesContainer>
      </EntityFocusProvider>,
    );
    expect(screen.getByTestId("perspective-count").textContent).toBe("2");
  });
});
