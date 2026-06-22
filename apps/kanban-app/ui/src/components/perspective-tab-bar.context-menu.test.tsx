import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { TooltipProvider } from "@/components/ui/tooltip";

/**
 * Context-menu scope-chain tests for {@link PerspectiveTabBar}.
 *
 * These live in a separate file from `perspective-tab-bar.test.tsx` because
 * that file mocks `@/lib/context-menu` wholesale — which would short-circuit
 * the exact contract we're verifying here (the real `useContextMenu` reads
 * `CommandScopeContext` and propagates the chain into the `window` server's
 * `show context menu` op).
 *
 * Pairs with `swissarmyhammer-kanban/tests/perspective_context_menu_integration.rs`
 * on the backend side — together they cover the full right-click →
 * scope-chain → resolver loop.
 */

import { answerListCommand } from "@/test/mock-command-list";

/** Tooltip open delay (ms) passed to `TooltipProvider` in these tests. */
const TOOLTIP_DELAY_MS = 100;

/**
 * Delay (ms) allowed for the right-click → scope-chain → `show context menu`
 * round-trip to settle before asserting on the captured items.
 */
const CONTEXT_MENU_SYNC_DELAY_MS = 20;

// `useContextMenu` fetches the Command registry at right-click time
// (`list command` via `command_tool_call`); drive it through `mockRegistry`.
let mockRegistry: Array<Record<string, unknown>> = [];

// Mock Tauri APIs before importing any modules that use them.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn<(...args: any[]) => Promise<unknown>>(
  (cmd, args) =>
    answerListCommand(cmd, args, mockRegistry) ?? Promise.resolve(null),
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
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { FocusLayer } from "./focus-layer";
import { asSegment } from "@/types/spatial";

/**
 * Publish a fixed set of context-menu commands through the registry — enough
 * to trigger the `show context menu` call path.
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
  // Publish through the registry. `perspective.*` commands are scoped to
  // `entity:perspective` so they match a chain carrying a `perspective:<id>`
  // moniker (the scope-expression → moniker rule in `useContextMenu`).
  mockRegistry = commands.map((c) => ({
    id: c.id,
    name: c.name,
    context_menu: c.context_menu,
    scope: ["entity:perspective"],
  }));
}

/**
 * The captured right-click scope chain. With the registry-driven context menu
 * the chain is written into the `show context menu` items' `scope_chain`, so
 * read it from the first non-separator item.
 */
function capturedListScope(): string[] | undefined {
  const scopes = capturedItemScopes();
  return scopes[0];
}

/**
 * Extract the scope chain(s) written into the `show context menu` items.
 *
 * The hook now reaches the `window` MCP server via
 * `callMcpTool("window", "show context menu", { items })`, which lowers onto
 * `invoke("command_tool_call", { module, tool, op, params })`. The items
 * therefore ride in that bridge call's `params.items`.
 */
function capturedItemScopes(): string[][] {
  return capturedItems().map((i) => i.scope_chain);
}

/** One context-menu item as written into the `show context menu` op. */
interface CapturedItem {
  name: string;
  cmd: string;
  target?: string;
  scope_chain: string[];
  separator: boolean;
}

/**
 * Extract every non-separator item written into the `show context menu` call.
 *
 * Items carry the self-contained dispatch info (`name`, `cmd`, `target`,
 * `scope_chain`) — the regression guard reads `cmd` / `name` / `scope_chain`
 * to prove a backend row surfaces and dispatches the right command.
 */
function capturedItems(): CapturedItem[] {
  const showCall = mockInvoke.mock.calls.find(
    (c) =>
      c[0] === "command_tool_call" &&
      (c[1] as { tool?: string })?.tool === "window" &&
      (c[1] as { op?: string })?.op === "show context menu",
  );
  const items =
    (
      (showCall?.[1] as { params?: unknown } | undefined)?.params as
        | { items?: CapturedItem[] }
        | undefined
    )?.items ?? [];
  return items.filter((i) => !i.separator);
}

/**
 * Render the tab bar inside a `window:main` ancestor scope so the
 * captured chain is realistic. Wraps in the spatial provider stack
 * since `PerspectiveTabBar` mounts a `<FocusScope>` and the
 * no-spatial-context fallback was removed in card
 * `01KQPVA127YMJ8D7NB6M824595`.
 */
function renderTabBarWithWindowScope() {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <TooltipProvider delayDuration={TOOLTIP_DELAY_MS}>
            <CommandScopeProvider moniker="window:main">
              <PerspectiveTabBar />
            </CommandScopeProvider>
          </TooltipProvider>
        </EntityFocusProvider>
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

  it("includes the tab's perspective moniker in the scope chain written into the context menu items", async () => {
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
      await new Promise((r) => setTimeout(r, CONTEXT_MENU_SYNC_DELAY_MS));
    });

    const chain = capturedListScope();
    expect(chain).toBeDefined();
    // Innermost first: perspective_tab:p2 → perspective:p2 → window:main.
    // Post-reshape (card 01KQQSVS4EBKKFN5SS7MW5P8CN) `perspective_tab:<id>`
    // comes from the `<FocusScope>` wrapper, not the inner FocusScope leaf
    // (which is `perspective_tab.name:<id>`). chain[0] still resolves to
    // `perspective_tab:p2` because `useContextMenu` is captured at
    // `PerspectiveTab`'s render scope — outside the inner
    // `<FocusScope perspective_tab.name:${id}>` — so the innermost segment
    // visible at capture time is the wrapping tab zone. `perspective:<id>`
    // continues to come from the surrounding `<CommandScopeProvider>`.
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
      await new Promise((r) => setTimeout(r, CONTEXT_MENU_SYNC_DELAY_MS));
    });

    const chains = capturedItemScopes();
    expect(chains.length).toBeGreaterThan(0);
    for (const chain of chains) {
      // Innermost: perspective_tab:p1 (the `<FocusScope>` wrapper, NOT the
      // inner `<FocusScope perspective_tab.name:p1>` leaf — `useContextMenu`
      // is captured outside that inner leaf) → perspective:p1 (the
      // surrounding `<CommandScopeProvider>`).
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
      await new Promise((r) => setTimeout(r, CONTEXT_MENU_SYNC_DELAY_MS));
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
// "Switch to Perspective «name»" context-menu row
// (card 01KV8SQR5VYH3B9GDK8QSMK7Z7).
//
// The backend `emit_perspective_goto` now flips the in-scope perspective's
// `perspective.switch` row to `context_menu: true`, so right-clicking a tab
// surfaces exactly its own "Switch to Perspective «name»" entry. This guard
// fails before that backend fix (no perspective row was ever
// `context_menu: true`, so the registry-driven menu dropped it). It dispatches
// `perspective.switch`; the right-clicked perspective id rides in the item's
// `scope_chain` (`perspective:<id>`), which the backend resolves via
// `ResolvedFrom::Scope`.
// ---------------------------------------------------------------------------

describe("PerspectiveTabBar 'Switch to Perspective' context-menu row", () => {
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

  it("renders the in-scope tab's 'Switch to Perspective «name»' item dispatching perspective.switch with that perspective's moniker", async () => {
    // Mirror the backend: the in-scope perspective's `perspective.switch` row
    // is `context_menu: true`. Right-clicking p2's tab must surface exactly
    // its "Switch to Perspective Second" row.
    mockResolvedCommands([
      {
        id: "perspective.switch",
        name: "Switch to Perspective Second",
        group: "perspective",
        context_menu: true,
        available: true,
      },
    ]);

    renderTabBarWithWindowScope();

    await act(async () => {
      fireEvent.contextMenu(screen.getByText("Second"));
      await new Promise((r) => setTimeout(r, CONTEXT_MENU_SYNC_DELAY_MS));
    });

    const items = capturedItems();
    const switchItem = items.find((i) => i.cmd === "perspective.switch");
    expect(switchItem).toBeDefined();
    expect(switchItem!.name).toBe("Switch to Perspective Second");
    // The right-clicked perspective id rides in the scope chain — the backend
    // resolves perspective_id from `perspective:p2` (ResolvedFrom::Scope).
    expect(switchItem!.scope_chain).toContain("perspective:p2");
    expect(switchItem!.target).toBe("perspective_tab:p2");
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
          <EntityFocusProvider>
            <TooltipProvider delayDuration={TOOLTIP_DELAY_MS}>
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
          </EntityFocusProvider>
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
      await new Promise((r) => setTimeout(r, CONTEXT_MENU_SYNC_DELAY_MS));
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
      await new Promise((r) => setTimeout(r, CONTEXT_MENU_SYNC_DELAY_MS));
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
    // A global (unscoped) command so the menu still renders and we can
    // inspect the captured chain. The registry-driven context menu only
    // calls `show context menu` when at least one command matches.
    mockRegistry = [{ id: "app.help", name: "Help", context_menu: true }];

    renderWithBodyChild();

    await act(async () => {
      fireEvent.contextMenu(screen.getByTestId("view-body"));
      await new Promise((r) => setTimeout(r, CONTEXT_MENU_SYNC_DELAY_MS));
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

interface ViewBodyWithContextMenuProps {
  testId: string;
}

function ViewBodyWithContextMenu({ testId }: ViewBodyWithContextMenuProps) {
  const handleContextMenu = useContextMenu();
  return (
    <div data-testid={testId} onContextMenu={handleContextMenu}>
      view body
    </div>
  );
}
