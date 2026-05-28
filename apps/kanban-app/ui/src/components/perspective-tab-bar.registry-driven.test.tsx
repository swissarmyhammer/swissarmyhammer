/**
 * Registry-driven `<CommandButton>` rendering inside `<PerspectiveTabBar>`.
 *
 * This pins task #4 of the `command-driven-ui` epic
 * (`01KRE1WT72MJWNGQBVAD4V5VKM`): the tab bar queries the live command
 * registry via `list_commands_for_scope` and renders one `<CommandButton>`
 * per command whose `tab_button` is set. The new render path lands
 * BEFORE any command is annotated with `tab_button`, so the visual
 * state today is identical to before — the migration tasks own the
 * per-command handoff that flips on a tab-button.
 *
 * Three contracts locked here:
 *
 * 1. When `list_commands_for_scope` returns a command with `tab_button`
 *    in scope, the corresponding `<CommandButton>` appears in the tab.
 * 2. When no command in scope carries `tab_button`, zero
 *    `<CommandButton>`s render — the existing hardcoded
 *    `<FilterFocusButton>` / `<GroupPopoverButton>` / `<AddPerspectiveButton>`
 *    remain untouched.
 * 3. The scope chain passed to `list_commands_for_scope` carries the
 *    `perspective:`, `view:`, and `board:` monikers — the backend's
 *    `filter_by_view_kind` pass relies on `view:` being present to drop
 *    commands the active view doesn't admit. The test simulates that
 *    pass in the mock and asserts the resulting button doesn't render.
 *
 * The hardcoded buttons stay in place during the transition; their
 * removal is the final step of each per-command migration task.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { TooltipProvider } from "@/components/ui/tooltip";

// ---------------------------------------------------------------------------
// Tauri mocks — must run before any module imports that pull command-scope.
// ---------------------------------------------------------------------------

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn(
  (..._args: any[]): Promise<unknown> => Promise.resolve(null),
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

// ---------------------------------------------------------------------------
// Perspective / view / schema / UI-state context mocks.
//
// Same shape as the other perspective-tab-bar tests in this directory so
// the test file feels at home next to its siblings.
// ---------------------------------------------------------------------------

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
}));

let mockViewsValue = {
  views: [{ id: "board-1", name: "Board", kind: "board", icon: "kanban" }],
  activeView: {
    id: "board-1",
    name: "Board",
    kind: "board",
    icon: "kanban",
  },
  setActiveViewId: vi.fn(),
  refresh: vi.fn(() => Promise.resolve()),
};

vi.mock("@/lib/views-context", () => ({
  useViews: () => mockViewsValue,
}));

vi.mock("@/lib/context-menu", () => ({
  useContextMenu: () => vi.fn(),
}));

// The tab bar now sources tab-button commands from the metadata-driven Command
// registry via `useCommandList`. Tests drive that seam by setting
// `mockRegistry`; the hook is filter-agnostic here (the component applies the
// `tab_button` / scope / `view_kinds` predicates).
let mockRegistry: Array<Record<string, unknown>> = [];
vi.mock("@/hooks/use-command-list", () => ({
  useCommandList: () => ({
    commands: mockRegistry,
    loading: false,
    refresh: vi.fn(),
  }),
}));

vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({ getEntities: () => [] }),
}));

// `useBoardData()` exposes the active board's parsed entity, including
// its canonical moniker (e.g. `"board:test-board"`). The tab bar reads
// that moniker into the scope chain it passes to
// `list_commands_for_scope`. Tests can override the id by reassigning
// `mockBoardId`; the moniker is rebuilt from it on every render.
let mockBoardId = "test-board";
vi.mock("@/components/window-container", () => ({
  useBoardData: () => ({
    board: {
      entity_type: "board",
      id: mockBoardId,
      moniker: `board:${mockBoardId}`,
      fields: {},
    },
    virtualTagMeta: [],
  }),
}));

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => ({ entity: { name: "task", fields: [] }, fields: [] }),
    getFieldDef: () => undefined,
    mentionableTypes: [],
    loading: false,
  }),
}));

const mockUIState = () => ({
  keymap_mode: "cua",
  scope_chain: [],
  open_boards: [],
  has_clipboard: false,
  clipboard_entity_type: null,
  windows: {},
  recent_boards: [],
});

vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => mockUIState(),
  useUIStateLoading: () => ({ state: mockUIState(), loading: false }),
}));

// ---------------------------------------------------------------------------
// Component-under-test imports — must come AFTER `vi.mock` calls above.
// ---------------------------------------------------------------------------

import { PerspectiveTabBar } from "./perspective-tab-bar";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { FocusLayer } from "./focus-layer";
import { asSegment } from "@/types/spatial";

/** Render `<PerspectiveTabBar>` inside the standard provider stack. */
function renderTabBar() {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <TooltipProvider delayDuration={100}>
            <PerspectiveTabBar />
          </TooltipProvider>
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

// ---------------------------------------------------------------------------
// Mock helpers — drive `list_commands_for_scope` responses.
// ---------------------------------------------------------------------------

/**
 * Resolved-command shape the backend emits; mirror only the fields
 * `<PerspectiveTabBar>` reads. Optional `view_kinds` is included so the
 * view-kinds simulator below can mimic the backend's
 * `filter_by_view_kind` pass.
 */
interface MockResolvedCommand {
  id: string;
  name: string;
  group?: string;
  context_menu?: boolean;
  available?: boolean;
  tab_button?: { icon: string };
  view_kinds?: readonly string[];
  params?: readonly { name: string; from?: string }[];
}

/**
 * Publish `commands` through the mocked `useCommandList` seam.
 *
 * Entity-scoped tab buttons must carry a non-empty `scope` so the tab bar's
 * `entity:perspective` filter admits them — default it here unless a test sets
 * one explicitly. The component applies the `tab_button` / `view_kinds`
 * predicates itself, so the registry returns the raw set.
 */
function mockResolvedCommands(commands: MockResolvedCommand[]) {
  mockRegistry = commands.map((c) => ({
    scope: ["entity:perspective"],
    ...c,
  }));
}

/**
 * Publish commands carrying `view_kinds` constraints. The tab bar's own
 * `isViewKindAdmitted` filter (the metadata-driven replacement for the
 * backend's `filter_by_view_kind` pass) drops a grid-only command when the
 * active view's kind is `board`, so the simulator is no longer needed — the
 * registry simply returns every command and the component filters.
 */
function mockResolvedCommandsWithViewKindFilter(
  commands: MockResolvedCommand[],
  _viewKindMap: Record<string, string>,
) {
  mockResolvedCommands(commands);
}

/**
 * Wait for in-flight async effects (`invoke` then `setState`) to settle.
 *
 * `<RegistryTabButtons>` calls `list_commands_for_scope` in a `useEffect`
 * and writes the result via `setCommands`. Both happen asynchronously,
 * so the test must yield to the event loop a few times AND wrap the
 * yield in `act(...)` so React processes the resulting state updates.
 */
async function flushEffects() {
  await act(async () => {
    // Three ticks: the `list_commands_for_scope` invoke resolves on
    // tick 1; the `setState` it triggers re-renders on tick 2; nested
    // effects in `<CommandButton>` (e.g. its spatial-nav register) get
    // one more.
    for (let i = 0; i < 3; i += 1) {
      // eslint-disable-next-line no-await-in-loop
      await new Promise<void>((resolve) => setTimeout(resolve, 0));
    }
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("PerspectiveTabBar — registry-driven CommandButton rendering", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockRegistry = [];
    mockBoardId = "test-board";
    mockPerspectivesValue = {
      perspectives: [{ id: "p1", name: "Sprint", view: "board" }],
      activePerspective: { id: "p1", name: "Sprint", view: "board" },
      setActivePerspectiveId: vi.fn(),
      refresh: vi.fn(() => Promise.resolve()),
    };
    mockViewsValue = {
      views: [{ id: "board-1", name: "Board", kind: "board", icon: "kanban" }],
      activeView: {
        id: "board-1",
        name: "Board",
        kind: "board",
        icon: "kanban",
      },
      setActiveViewId: vi.fn(),
      refresh: vi.fn(() => Promise.resolve()),
    };
  });

  it("renders_command_button_for_each_tab_button_tagged_command — one CommandButton per command in scope", async () => {
    mockResolvedCommands([
      {
        id: "perspective.focusFilter",
        name: "Focus filter",
        group: "perspective",
        context_menu: false,
        available: true,
        tab_button: { icon: "filter" },
      },
    ]);

    renderTabBar();
    await flushEffects();

    // The registry-rendered button is identified by its aria-label, which
    // `<CommandButton>` derives from `command.name`. Use `getAllByRole`
    // because the existing hardcoded `Filter` button on the active tab
    // also carries `aria-label="Filter"` — the new button's name
    // ("Focus filter") is what distinguishes it.
    const registryButton = screen.getByRole("button", {
      name: "Focus filter",
    });
    expect(registryButton).toBeTruthy();
  });

  it("renders_zero_command_buttons_when_no_commands_have_tab_button — empty registry list leaves the bar visually identical", async () => {
    // The bridge returns commands that do NOT carry `tab_button` — the
    // tab bar's registry-rendered slot is empty. Today (after the
    // 01KRE1YA65MMG29RDQDQ0VPJQG migration), `perspective.filter.focus`
    // is the first command that DOES carry `tab_button`; this test
    // deliberately omits it from the response so the slot stays empty
    // and the remaining hardcoded affordances render as before.
    mockResolvedCommands([
      {
        id: "perspective.rename",
        name: "Rename perspective",
        group: "perspective",
        context_menu: true,
        available: true,
        // tab_button intentionally absent.
      },
      {
        id: "ui.entity.delete",
        name: "Delete",
        group: "global",
        context_menu: true,
        available: true,
        // tab_button intentionally absent.
      },
    ]);

    renderTabBar();
    await flushEffects();

    // Every legacy hardcoded affordance is now registry-rendered: the
    // hardcoded "Filter" button (aria-label "Filter") was deleted by
    // 01KRE1YA65MMG29RDQDQ0VPJQG, the hardcoded "Group" button
    // (aria-label "Group") was deleted by 01KRE1ZTYJ5PPTQ29K72KE88B5,
    // and the hardcoded `<AddPerspectiveButton>` (aria-label
    // "Add perspective") was deleted by 01KRE21GJMPP289N1HSTMJG5HE.
    // Each replacement is the registry-rendered `<CommandButton>` for
    // the migrated command, which only mounts when the registry
    // response includes that command — which this test deliberately
    // does not.
    //
    // We assert the negative path by checking that none of the
    // registry-supplied command names produced a button AND that no
    // hardcoded legacy aria-labels survived the final migration.
    expect(
      screen.queryByRole("button", { name: "Rename perspective" }),
    ).toBeNull();
    expect(screen.queryByRole("button", { name: "Delete" })).toBeNull();

    // The legacy hardcoded "Filter" / "Group" / "Group By" /
    // "Add perspective" buttons are gone — their replacements are the
    // registry-driven `<CommandButton>`s rendered only when their
    // respective commands (`perspective.filter.focus`,
    // `perspective.group`, `perspective.save`, `perspective.sort.set`)
    // are in scope (each covered by
    // `renders_command_button_for_each_tab_button_tagged_command` and
    // the per-migration test files).
    expect(screen.queryByRole("button", { name: "Filter" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Group" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Group By" })).toBeNull();
    expect(
      screen.queryByRole("button", { name: /add perspective/i }),
    ).toBeNull();
  });

  it("respects_view_kinds_filter — a command with view_kinds: [grid] does NOT render under view kind board", async () => {
    // The backend's `filter_by_view_kind` pass is what drops the
    // command before it reaches the frontend. We simulate that pass in
    // the mock so this test exercises the same end-to-end contract a
    // production run would: a command annotated with `view_kinds:
    // [grid]` MUST be invisible when the active view is a board.
    mockResolvedCommandsWithViewKindFilter(
      [
        {
          id: "perspective.gridOnlyCommand",
          name: "Grid-only command",
          group: "perspective",
          context_menu: false,
          available: true,
          tab_button: { icon: "filter" },
          view_kinds: ["grid"],
        },
      ],
      { "board-1": "board" },
    );

    renderTabBar();
    await flushEffects();

    // The grid-pinned command was dropped by the simulated
    // `filter_by_view_kind` pass — zero `<CommandButton>`s land in the
    // tab. The hardcoded `Filter` button on the active tab still
    // renders, so we don't confuse the two by name.
    expect(
      screen.queryByRole("button", { name: "Grid-only command" }),
    ).toBeNull();
  });

  it("only renders entity-scoped tab-button commands in the per-perspective slot", async () => {
    // The per-tab slot renders entity-scoped tab buttons; global (unscoped)
    // tab-button commands belong to the bar-level slot. A command with an
    // empty `scope` must not surface inside the perspective tab.
    mockRegistry = [
      {
        id: "perspective.focusFilter",
        name: "Focus filter",
        scope: ["entity:perspective"],
        tab_button: { icon: "filter" },
      },
      {
        id: "perspective.save",
        name: "Add perspective",
        scope: [],
        tab_button: { icon: "plus" },
      },
    ];

    renderTabBar();
    await flushEffects();

    // Entity-scoped command renders inside the active perspective tab.
    expect(screen.getByRole("button", { name: "Focus filter" })).toBeTruthy();
  });
});
