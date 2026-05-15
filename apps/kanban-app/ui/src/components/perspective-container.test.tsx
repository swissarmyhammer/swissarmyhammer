import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { useContext } from "react";
import { CommandScopeContext, scopeChainFromScope } from "@/lib/command-scope";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import type { PerspectiveDef, Entity } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Mocks — Tauri APIs must be mocked before importing components.
// ---------------------------------------------------------------------------

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve(null)),
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
// Mock perspective-context — PerspectiveContainer reads the active perspective.
// ---------------------------------------------------------------------------

const mockUsePerspectives = vi.hoisted(() =>
  vi.fn(() => ({
    perspectives: [] as PerspectiveDef[],
    activePerspective: null as PerspectiveDef | null,
    setActivePerspectiveId: vi.fn(),
    refresh: vi.fn(),
  })),
);

vi.mock("@/lib/perspective-context", () => ({
  usePerspectives: () => mockUsePerspectives(),
}));

// Mock ui-state-context for transitive dependencies. `UIStateProvider` is
// referenced through the rust-engine container in test wrappers.
vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => ({ windows: {} }),
  UIStateProvider: ({ children }: { children: unknown }) => children,
}));

// Spy on `useRefreshEntities` so we can prove `PerspectiveContainer` does
// NOT call it on perspective change after 01KP3ERHEDP86C2JYYR7NM1593 — the
// backend's `perspective.switch` now owns filter evaluation and writes the
// matching task ids straight into `UIState.filtered_task_ids`. Returning a
// `vi.fn()` here gives the regression test below a stable spy handle.
const mockRefreshEntities = vi.fn(() => Promise.resolve({ entities: {} }));
vi.mock("@/components/rust-engine-container", () => ({
  useRefreshEntities: () => mockRefreshEntities,
}));

// Import after mocks
import {
  PerspectiveContainer,
  useActivePerspective,
} from "./perspective-container";

// ---------------------------------------------------------------------------
// Probes
// ---------------------------------------------------------------------------

/** Reads the scope chain and renders monikers. */
function ScopeProbe() {
  const scope = useContext(CommandScopeContext);
  const chain = scope ? scopeChainFromScope(scope) : [];
  return <span data-testid="scope-chain">{chain.join(",")}</span>;
}

/** Reads the active perspective context and renders its ID. */
function PerspectiveProbe() {
  const { activePerspective } = useActivePerspective();
  return (
    <span data-testid="active-perspective-id">
      {activePerspective?.id ?? "none"}
    </span>
  );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("PerspectiveContainer", () => {
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
        <PerspectiveContainer>
          <span data-testid="child">hello</span>
        </PerspectiveContainer>
      </EntityFocusProvider>,
    );
    expect(screen.getByTestId("child").textContent).toBe("hello");
  });

  it("provides a perspective:{id} scope moniker when active perspective exists", () => {
    const perspective: PerspectiveDef = {
      id: "p1",
      name: "Default",
      view: "board",
    };
    mockUsePerspectives.mockReturnValue({
      perspectives: [perspective],
      activePerspective: perspective,
      setActivePerspectiveId: vi.fn(),
      refresh: vi.fn(),
    });

    render(
      <EntityFocusProvider>
        <PerspectiveContainer>
          <ScopeProbe />
        </PerspectiveContainer>
      </EntityFocusProvider>,
    );

    const chain = screen.getByTestId("scope-chain").textContent!;
    expect(chain).toContain("perspective:p1");
  });

  it("uses perspective:default moniker when no active perspective", () => {
    render(
      <EntityFocusProvider>
        <PerspectiveContainer>
          <ScopeProbe />
        </PerspectiveContainer>
      </EntityFocusProvider>,
    );

    const chain = screen.getByTestId("scope-chain").textContent!;
    expect(chain).toContain("perspective:default");
  });

  it("provides active perspective data via useActivePerspective context", () => {
    const perspective: PerspectiveDef = {
      id: "p2",
      name: "Bugs Only",
      view: "board",
      filter: "#bug",
    };
    mockUsePerspectives.mockReturnValue({
      perspectives: [perspective],
      activePerspective: perspective,
      setActivePerspectiveId: vi.fn(),
      refresh: vi.fn(),
    });

    render(
      <EntityFocusProvider>
        <PerspectiveContainer>
          <PerspectiveProbe />
        </PerspectiveContainer>
      </EntityFocusProvider>,
    );

    expect(screen.getByTestId("active-perspective-id").textContent).toBe("p2");
  });

  it("provides applySort helper via context", () => {
    const perspective: PerspectiveDef = {
      id: "p1",
      name: "Default",
      view: "board",
      sort: [{ field: "Title", direction: "asc" }],
    };
    mockUsePerspectives.mockReturnValue({
      perspectives: [perspective],
      activePerspective: perspective,
      setActivePerspectiveId: vi.fn(),
      refresh: vi.fn(),
    });

    const entities: Entity[] = [
      {
        id: "t1",
        entity_type: "task",
        moniker: "task:t1",
        fields: { Title: "B" },
      },
      {
        id: "t2",
        entity_type: "task",
        moniker: "task:t2",
        fields: { Title: "A" },
      },
    ];

    /** Probe that applies sort. */
    function SortProbe() {
      const { applySort } = useActivePerspective();
      const sorted = applySort(entities);
      return (
        <span data-testid="result">{sorted.map((e) => e.id).join(",")}</span>
      );
    }

    render(
      <EntityFocusProvider>
        <PerspectiveContainer>
          <SortProbe />
        </PerspectiveContainer>
      </EntityFocusProvider>,
    );

    // Sort by Title asc: A before B
    expect(screen.getByTestId("result").textContent).toBe("t2,t1");
  });

  // Regression guard for 01KP3ERHEDP86C2JYYR7NM1593: the old container
  // chased perspective changes with a `refreshEntities(boardPath,
  // activeFilter)` roundtrip in a `useEffect`. That two-step dance is
  // gone — `perspective.switch` (dispatched from `perspective-context`)
  // now writes the filtered task ids into UIState atomically, and the
  // view layer reads them from there. So no matter how the active
  // perspective (or its filter) changes, PerspectiveContainer must
  // NEVER reach for `refreshEntities`.
  it("does not call refreshEntities on perspective change", () => {
    const before: PerspectiveDef = {
      id: "p1",
      name: "Bugs",
      view: "board",
      filter: "#bug",
    };
    mockUsePerspectives.mockReturnValue({
      perspectives: [before],
      activePerspective: before,
      setActivePerspectiveId: vi.fn(),
      refresh: vi.fn(),
    });

    const { rerender } = render(
      <EntityFocusProvider>
        <PerspectiveContainer>
          <span data-testid="child">hi</span>
        </PerspectiveContainer>
      </EntityFocusProvider>,
    );

    // Mount must not trigger a refresh — the UIState snapshot already
    // carries the correct filtered_task_ids for the active perspective.
    expect(mockRefreshEntities).not.toHaveBeenCalled();

    // Now flip the active perspective (and its filter). The container
    // must STILL not call refresh — only the dispatch from
    // perspective-context's `setActivePerspectiveId` does any work, and
    // that path goes through `perspective.switch`, not through
    // `refreshEntities`.
    const after: PerspectiveDef = {
      id: "p2",
      name: "Features",
      view: "board",
      filter: "#feature",
    };
    mockUsePerspectives.mockReturnValue({
      perspectives: [after],
      activePerspective: after,
      setActivePerspectiveId: vi.fn(),
      refresh: vi.fn(),
    });

    rerender(
      <EntityFocusProvider>
        <PerspectiveContainer>
          <span data-testid="child">hi</span>
        </PerspectiveContainer>
      </EntityFocusProvider>,
    );

    expect(mockRefreshEntities).not.toHaveBeenCalled();
  });

  it("provides groupField from the active perspective", () => {
    const perspective: PerspectiveDef = {
      id: "p1",
      name: "Default",
      view: "board",
      group: "Status",
    };
    mockUsePerspectives.mockReturnValue({
      perspectives: [perspective],
      activePerspective: perspective,
      setActivePerspectiveId: vi.fn(),
      refresh: vi.fn(),
    });

    function GroupProbe() {
      const { groupField } = useActivePerspective();
      return <span data-testid="group-field">{groupField ?? "none"}</span>;
    }

    render(
      <EntityFocusProvider>
        <PerspectiveContainer>
          <GroupProbe />
        </PerspectiveContainer>
      </EntityFocusProvider>,
    );

    expect(screen.getByTestId("group-field").textContent).toBe("Status");
  });
});
