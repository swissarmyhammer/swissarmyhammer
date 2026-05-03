import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { TooltipProvider } from "@/components/ui/tooltip";

/**
 * Context-menu scope-chain tests for {@link PerspectiveTabBar}.
 *
 * These live in a separate file from `perspective-tab-bar.test.tsx` because
 * that file mocks `@/lib/context-menu` wholesale — which would short-circuit
 * the exact contract we're verifying here (the real `useContextMenu` reads
 * `CommandScopeContext` and propagates the chain into `list_commands_for_scope`
 * + `show_context_menu`).
 *
 * Pairs with `swissarmyhammer-kanban/tests/perspective_context_menu_integration.rs`
 * on the backend side — together they cover the full right-click →
 * scope-chain → resolver loop.
 */

// Mock Tauri APIs before importing any modules that use them.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn<(...args: any[]) => Promise<unknown>>(() =>
  Promise.resolve(null),
);
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

/** Shape of a perspective in the mock context. */
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
  PerspectiveProvider: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
}));

const mockViewsValue = {
  views: [{ id: "board-1", name: "Board", kind: "board", icon: "kanban" }],
  activeView: { id: "board-1", name: "Board", kind: "board", icon: "kanban" },
  setActiveViewId: vi.fn(),
  refresh: vi.fn(() => Promise.resolve()),
};

vi.mock("@/lib/views-context", () => ({
  useViews: () => mockViewsValue,
}));

// Entity store stub (needed transitively by FilterEditor).
vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({ getEntities: () => [] }),
}));

// Board data stub (virtual tag metadata).
vi.mock("@/components/window-container", () => ({
  useBoardData: () => ({ virtualTagMeta: [] }),
}));

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => ({ entity: { name: "task", fields: [] }, fields: [] }),
    getFieldDef: () => undefined,
    mentionableTypes: [],
    loading: false,
  }),
}));

vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => ({
    keymap_mode: "cua",
    scope_chain: [],
    open_boards: [],
    has_clipboard: false,
    clipboard_entity_type: null,
    windows: {},
    recent_boards: [],
  }),
  useUIStateLoading: () => ({
    state: {
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      has_clipboard: false,
      clipboard_entity_type: null,
      windows: {},
      recent_boards: [],
    },
    loading: false,
  }),
}));

import { PerspectiveTabBar } from "./perspective-tab-bar";
import { PerspectivesContainer } from "./perspectives-container";
import { CommandScopeProvider } from "@/lib/command-scope";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "./focus-layer";
import { asSegment } from "@/types/spatial";

/**
 * Drive the mocked `invoke` so `list_commands_for_scope` returns a fixed
 * set of commands — enough to trigger the `show_context_menu` call path.
 */
function mockResolvedCommands(
  commands: Array<{
    id: string;
    name: string;
    group: string;
    context_menu: boolean;
    available: boolean;
  }>,
) {
  mockInvoke.mockImplementation((cmd: string, _args?: unknown) => {
    if (cmd === "list_commands_for_scope") return Promise.resolve(commands);
    return Promise.resolve(undefined);
  });
}

/** Extract the scope chain that was passed to `list_commands_for_scope`. */
function capturedListScope(): string[] | undefined {
  const listCall = mockInvoke.mock.calls.find(
    (c) => c[0] === "list_commands_for_scope",
  );
  return (listCall?.[1] as { scopeChain?: string[] } | undefined)?.scopeChain;
}

/** Extract the scope chain(s) that were written into `show_context_menu` items. */
function capturedItemScopes(): string[][] {
  const showCall = mockInvoke.mock.calls.find(
    (c) => c[0] === "show_context_menu",
  );
  const items =
    (
      showCall?.[1] as
        | {
            items?: Array<{ scope_chain: string[]; separator: boolean }>;
          }
        | undefined
    )?.items ?? [];
  return items.filter((i) => !i.separator).map((i) => i.scope_chain);
}

/**
 * Render the tab bar inside a `window:main` ancestor scope so the
 * captured chain is realistic. Wraps in the spatial provider stack
 * since `PerspectiveTabBar` mounts a `<FocusZone>` and the
 * no-spatial-context fallback was removed in card
 * `01KQPVA127YMJ8D7NB6M824595`.
 */
function renderTabBarWithWindowScope() {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <TooltipProvider delayDuration={100}>
          <CommandScopeProvider moniker="window:main">
            <PerspectiveTabBar />
          </CommandScopeProvider>
        </TooltipProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

describe("PerspectiveTabBar right-click scope chain", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockPerspectivesValue = {
      perspectives: [
        { id: "p1", name: "First", view: "board" },
        { id: "p2", name: "Second", view: "board" },
      ],
      activePerspective: { id: "p1", name: "First", view: "board" },
      setActivePerspectiveId: vi.fn(),
      refresh: vi.fn(() => Promise.resolve()),
    };
  });

  it("includes the tab's perspective moniker in the scope chain passed to list_commands_for_scope", async () => {
    mockResolvedCommands([
      {
        id: "perspective.clearFilter",
        name: "Clear Filter",
        group: "perspective",
        context_menu: true,
        available: true,
      },
    ]);

    renderTabBarWithWindowScope();

    await act(async () => {
      // Right-click on the non-active tab. The tab lives inside a
      // ScopedPerspectiveTab → CommandScopeProvider moniker="perspective:p2"
      // so the chain captured at right-click time should lead with `perspective:p2`.
      fireEvent.contextMenu(screen.getByText("Second"));
      await new Promise((r) => setTimeout(r, 20));
    });

    const chain = capturedListScope();
    expect(chain).toBeDefined();
    // Innermost first: perspective_tab:p2 → perspective:p2 → window:main
    // (`perspective_tab:<id>` comes from the inner `<FocusScope>` leaf;
    // `perspective:<id>` from the surrounding `<CommandScopeProvider>`.)
    expect(chain![0]).toBe("perspective_tab:p2");
    expect(chain).toContain("perspective:p2");
    expect(chain).toContain("window:main");
  });

  it("writes the perspective moniker into every ContextMenuItem scope_chain for a right-clicked tab", async () => {
    mockResolvedCommands([
      {
        id: "perspective.clearFilter",
        name: "Clear Filter",
        group: "perspective",
        context_menu: true,
        available: true,
      },
      {
        id: "perspective.clearGroup",
        name: "Clear Group",
        group: "perspective",
        context_menu: true,
        available: true,
      },
    ]);

    renderTabBarWithWindowScope();

    await act(async () => {
      fireEvent.contextMenu(screen.getByText("First"));
      await new Promise((r) => setTimeout(r, 20));
    });

    const chains = capturedItemScopes();
    expect(chains.length).toBeGreaterThan(0);
    for (const chain of chains) {
      // Innermost: perspective_tab:p1 (the `<FocusScope>` leaf) →
      // perspective:p1 (the surrounding `<CommandScopeProvider>`).
      expect(chain[0]).toBe("perspective_tab:p1");
      expect(chain).toContain("perspective:p1");
      expect(chain).toContain("window:main");
    }
  });

  it("right-clicking the non-active tab carries THAT perspective's moniker, not the active one", async () => {
    mockResolvedCommands([
      {
        id: "perspective.clearFilter",
        name: "Clear Filter",
        group: "perspective",
        context_menu: true,
        available: true,
      },
    ]);

    renderTabBarWithWindowScope();

    await act(async () => {
      fireEvent.contextMenu(screen.getByText("Second"));
      await new Promise((r) => setTimeout(r, 20));
    });

    // The active perspective is p1, but we right-clicked on p2's tab.
    // The scope chain must reflect p2, not p1 — this is the contract
    // that lets "Clear Filter" act on a non-active perspective.
    const chain = capturedListScope();
    expect(chain![0]).toBe("perspective_tab:p2");
    expect(chain).toContain("perspective:p2");
    expect(chain).not.toContain("perspective_tab:p1");
    expect(chain).not.toContain("perspective:p1");
  });
});

// ---------------------------------------------------------------------------
// View-body scope — PerspectivesContainer injects `perspective:<active-id>`
// ---------------------------------------------------------------------------
//
// Regression guard for the main bug in the task: right-clicks below the tab
// bar (grid rows, column headers, board canvas) now carry the active
// perspective's moniker in their scope chain so `resolve_perspective_id` on
// the backend picks `ResolvedFrom::Scope` instead of falling through to
// `UiState`.

describe("PerspectivesContainer view-body scope", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockPerspectivesValue = {
      perspectives: [{ id: "p-active", name: "Active", view: "board" }],
      activePerspective: { id: "p-active", name: "Active", view: "board" },
      setActivePerspectiveId: vi.fn(),
      refresh: vi.fn(() => Promise.resolve()),
    };
  });

  /**
   * Render `PerspectivesContainer` with a child that captures right-click
   * events from the view body. Wraps in the spatial provider stack since
   * the container mounts spatial primitives.
   */
  function renderWithBodyChild(bodyTestId = "view-body") {
    return render(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <TooltipProvider delayDuration={100}>
            <CommandScopeProvider moniker="window:main">
              <PerspectivesContainer>
                {/* Attach a passthrough onContextMenu via the real useContextMenu
                    hook by delegating to a child component. Keeping the hook
                    call inside the child means it reads the scope chain that
                    PerspectivesContainer's `ActivePerspectiveScope` injected. */}
                <ViewBodyWithContextMenu testId={bodyTestId} />
              </PerspectivesContainer>
            </CommandScopeProvider>
          </TooltipProvider>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
  }

  it("injects the active perspective moniker into right-clicks on the view body", async () => {
    mockResolvedCommands([
      {
        id: "perspective.clearFilter",
        name: "Clear Filter",
        group: "perspective",
        context_menu: true,
        available: true,
      },
    ]);

    renderWithBodyChild();

    await act(async () => {
      fireEvent.contextMenu(screen.getByTestId("view-body"));
      await new Promise((r) => setTimeout(r, 20));
    });

    const chain = capturedListScope();
    expect(chain).toBeDefined();
    expect(chain).toContain("perspective:p-active");
    expect(chain).toContain("window:main");
  });

  it("writes the active perspective moniker into every ContextMenuItem from the view body", async () => {
    mockResolvedCommands([
      {
        id: "perspective.clearFilter",
        name: "Clear Filter",
        group: "perspective",
        context_menu: true,
        available: true,
      },
      {
        id: "perspective.clearGroup",
        name: "Clear Group",
        group: "perspective",
        context_menu: true,
        available: true,
      },
    ]);

    renderWithBodyChild();

    await act(async () => {
      fireEvent.contextMenu(screen.getByTestId("view-body"));
      await new Promise((r) => setTimeout(r, 20));
    });

    const chains = capturedItemScopes();
    expect(chains.length).toBeGreaterThan(0);
    for (const chain of chains) {
      expect(chain).toContain("perspective:p-active");
      expect(chain).toContain("window:main");
    }
  });

  it("omits the perspective moniker entirely when no perspective is active", async () => {
    // When there is no active perspective, the container must NOT inject
    // a stale moniker. The backend's `scope: "entity:perspective"` filter
    // then hides every perspective.* command — which is the correct
    // behavior (there is nothing to mutate).
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      activePerspective: null,
    };
    mockResolvedCommands([]);

    renderWithBodyChild();

    await act(async () => {
      fireEvent.contextMenu(screen.getByTestId("view-body"));
      await new Promise((r) => setTimeout(r, 20));
    });

    const chain = capturedListScope();
    expect(chain).toBeDefined();
    for (const moniker of chain!) {
      expect(moniker.startsWith("perspective:")).toBe(false);
    }
  });
});

// ---------------------------------------------------------------------------
// ViewBodyWithContextMenu — tiny helper component that attaches the real
// `useContextMenu` handler to a div. Kept at file scope so it can close
// over the `useContextMenu` import without shadowing inside a describe.
// ---------------------------------------------------------------------------

import { useContextMenu } from "@/lib/context-menu";

function ViewBodyWithContextMenu({ testId }: { testId: string }) {
  const handleContextMenu = useContextMenu();
  return (
    <div data-testid={testId} onContextMenu={handleContextMenu}>
      view body
    </div>
  );
}
