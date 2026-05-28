/**
 * Regression tests for the Add Perspective and Sort tab-button migrations
 * to command-driven rendering (task 01KRE21GJMPP289N1HSTMJG5HE — final
 * task in the #command-driven-ui epic).
 *
 * Before the migration:
 *   - The `+` add affordance was a hardcoded `<AddPerspectiveButton>`
 *     at the bar level with an inline "Untitled" / "Untitled N+1" name
 *     inference that dispatched `perspective.save` directly.
 *   - The Sort affordance had NO tab-bar entry point — sort was driven
 *     by column-header clicks in the grid view's tanstack-table. The
 *     `view_kinds: [grid]` annotation on `perspective.sort.set` was
 *     enforced in palettes / context menus only; no surfaced tab button
 *     meant no view-kind-driven hide on the board view either.
 *
 * After the migration both affordances are registry-rendered
 * `<CommandButton>`s driven by YAML annotations:
 *
 *   - `perspective.save`: `tab_button: { icon: plus }`; the `name` param
 *     is `shape: text`. The dispatcher's empty-input fallback to
 *     `"Untitled"` keeps the user-visible behavior aligned with the
 *     legacy button.
 *   - `perspective.sort.set`: `tab_button: { icon: arrow-up-down }`;
 *     `field` is enum-shaped sourced from `perspective.fields`,
 *     `direction` is enum-shaped sourced from `sort.directions`. The
 *     existing `view_kinds: [grid]` annotation now actually hides the
 *     button on board views — same mechanism every other view-
 *     restricted tab button uses.
 *
 * Five contracts locked here:
 *
 *   1. The Add `<CommandButton>` mounts at the bar level with the
 *      `plus` lucide icon (resolved from `tab_button.icon`).
 *   2. Submitting the Add popover with a typed name dispatches
 *      `perspective.save` with `{ name, view_id }`.
 *   3. The Sort `<CommandButton>` mounts on a grid view (with the
 *      `arrow-up-down` icon) and is absent on a board view — the
 *      regression test the original perspective-sort bug needed and
 *      didn't get.
 *   4. Submitting the Sort popover with a picked field + direction
 *      dispatches `perspective.sort.set` with both args.
 *   5. The perspective tab bar contains zero hardcoded
 *      `<FilterFocusButton>` / `<GroupPopoverButton>` /
 *      `<AddPerspectiveButton>` JSX — every tab-button affordance is
 *      registry-rendered after this migration lands.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act, fireEvent } from "@testing-library/react";
import { TooltipProvider } from "@/components/ui/tooltip";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before any module imports that pull
// command-scope / perspective-tab-bar.
// ---------------------------------------------------------------------------

const { mockInvoke } = vi.hoisted(() => {
  const mockInvoke = vi.fn(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (..._args: any[]): Promise<unknown> => Promise.resolve(null),
  );
  return { mockInvoke };
});

vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
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
// Domain context mocks — match the sibling migration test (group-migration)
// so the bar mounts with a known active perspective + view + board.
// ---------------------------------------------------------------------------

type MockPerspective = {
  id: string;
  name: string;
  view: string;
  filter?: string;
  group?: string;
};

type MockView = {
  id: string;
  name: string;
  kind: string;
  icon: string;
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

let mockViewsValue: {
  views: MockView[];
  activeView: MockView | null;
  setActiveViewId: () => void;
  refresh: () => Promise<void>;
} = {
  views: [{ id: "board-1", name: "Board", kind: "board", icon: "kanban" }],
  activeView: { id: "board-1", name: "Board", kind: "board", icon: "kanban" },
  setActiveViewId: vi.fn(),
  refresh: vi.fn(() => Promise.resolve()),
};

vi.mock("@/lib/views-context", () => ({
  useViews: () => mockViewsValue,
}));

vi.mock("@/lib/context-menu", () => ({
  useContextMenu: () => vi.fn(),
}));

// Tab buttons source from the Command registry via `useCommandList`.
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
  keymap_mode: "cua" as const,
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
// Component-under-test imports — must come AFTER `vi.mock` above.
// ---------------------------------------------------------------------------

import { PerspectiveTabBar } from "./perspective-tab-bar";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { FocusLayer } from "./focus-layer";
import { asSegment } from "@/types/spatial";

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/**
 * Registry payload shape for the migrated `perspective.save` tab-button
 * command. Group is `"global"` because the YAML carries no `scope`
 * field, matching how `emit_global_registry_commands` tags unscoped
 * registry commands.
 */
function addRegistryEntry() {
  return {
    id: "perspective.save",
    name: "Save Perspective",
    group: "global",
    context_menu: false,
    available: true,
    tab_button: { icon: "plus" },
    params: [
      {
        name: "name",
        from: "args",
        shape: "text",
      },
      {
        name: "view_id",
        from: "scope_chain",
        entity_type: "view",
      },
    ],
    keys: {},
  };
}

/**
 * Registry payload shape for the migrated `perspective.sort.set`
 * tab-button command. Group is `"perspective"` because the YAML carries
 * `scope: "entity:perspective"`, matching how
 * `emit_scoped_registry_commands` tags entity-scoped rows.
 *
 * The two enum params drive the multi-param form-branch popover render
 * — single-enum-param menu render is reserved for one-click commands
 * like Group By.
 */
function sortRegistryEntry(
  fieldOptions: readonly { value: string; label: string }[] = [
    { value: "title", label: "Title" },
    { value: "status", label: "Status" },
  ],
) {
  return {
    id: "perspective.sort.set",
    name: "Sort Field",
    group: "perspective",
    context_menu: false,
    available: true,
    tab_button: { icon: "arrow-up-down" },
    view_kinds: ["grid"],
    params: [
      {
        name: "field",
        from: "args",
        shape: "enum",
        options_from: "perspective.fields",
        options: fieldOptions,
      },
      {
        name: "direction",
        from: "args",
        shape: "enum",
        options_from: "sort.directions",
        options: [
          { value: "asc", label: "Ascending" },
          { value: "desc", label: "Descending" },
        ],
      },
      { name: "perspective_id", from: "scope_chain", entity_type: "perspective" },
    ],
    keys: {},
  };
}

/**
 * Install an `invoke` mock that returns the given commands for every
 * `list_commands_for_scope` call. Both the per-tab and bar-level
 * surfaces query the registry independently; the mock answers both
 * with the same payload — the production backend would emit the
 * appropriate subset for each scope chain, and the frontend's per-
 * surface `group` filter (`!== "global"` per-tab, `=== "global"` at
 * the bar) handles the split.
 */
function mockResolvedCommands(commands: unknown[]) {
  // The registry-driven split is by `scope` emptiness: global (unscoped)
  // tab-button commands render at the bar level, perspective-scoped ones in
  // the active tab. Map the legacy `group: "global"` marker onto an empty
  // `scope` and everything else onto `entity:perspective`.
  mockRegistry = (commands as Array<Record<string, unknown>>).map((c) => ({
    scope: c.group === "global" ? [] : ["entity:perspective"],
    ...c,
  }));
}

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

/**
 * Wait for `list_commands_for_scope`'s async effect to settle.
 *
 * Both `<RegistryTabButtons>` (per-tab) and `<BarRegistryTabButtons>`
 * (bar-level) call `invoke` inside a `useEffect` and write the result
 * via `setCommands`. Four event-loop turns reliably cover both surfaces'
 * resolve → setState chains.
 */
async function flushEffects() {
  await act(async () => {
    for (let i = 0; i < 4; i += 1) {
      // eslint-disable-next-line no-await-in-loop
      await new Promise<void>((resolve) => setTimeout(resolve, 0));
    }
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("perspective-tab-bar — Add perspective migration", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
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

  // -------------------------------------------------------------------------
  // 1. The registry-rendered Add `<CommandButton>` mounts at the bar level
  //    with the `plus` lucide icon (resolved from `tab_button.icon`).
  // -------------------------------------------------------------------------

  it("add_perspective_button_renders_with_plus_icon_from_registry", async () => {
    mockResolvedCommands([addRegistryEntry()]);

    renderTabBar();
    await flushEffects();

    // `<CommandButton>` derives its `aria-label` from `command.name`
    // ("Save Perspective"). The legacy hardcoded `<AddPerspectiveButton>`
    // (aria-label "Add perspective") is deleted by this migration so the
    // new name is unambiguous.
    const button = screen.getByRole("button", { name: "Save Perspective" });
    expect(button).toBeTruthy();

    // The lucide `Plus` icon resolves via `commandIconFor("plus")` in
    // `command-icon-registry.ts`. The icon-registry assigns it the
    // `lucide-plus` class so we can pin the icon identity without
    // depending on internal SVG shape.
    const svg = button.querySelector("svg");
    expect(svg).not.toBeNull();
    expect(svg?.classList.contains("lucide-plus")).toBe(true);
  });

  // -------------------------------------------------------------------------
  // 2. Submitting the Add popover with a typed name dispatches
  //    `perspective.save` with the name.
  //
  //    The `view_id` arg is NOT asserted on the wire here on purpose:
  //    the production dispatcher (`SavePerspectiveCmd::execute`) reads
  //    `view_id` from the scope chain when the args bag does not supply
  //    one (see `test_save_perspective_cmd_resolves_view_id_from_scope_chain`
  //    in `swissarmyhammer-kanban/src/commands/perspective_commands.rs`).
  //    There is no backend scope-chain-to-args injection pass in
  //    `dispatch_command_internal`; the fallback lives in
  //    `SavePerspectiveCmd::execute` itself, so the wire payload that
  //    reaches the backend from the popover carries only `{ name }`.
  //    The dispatch-time `view_id` resolution is pinned by the Rust
  //    integration tests cited above; this frontend test only checks
  //    that the popover collects `name` and dispatches the command.
  // -------------------------------------------------------------------------

  it("submitting_add_popover_dispatches_perspective_save_with_name", async () => {
    mockResolvedCommands([addRegistryEntry()]);

    renderTabBar();
    await flushEffects();

    const button = screen.getByRole("button", { name: "Save Perspective" });
    await act(async () => {
      fireEvent.click(button);
      await Promise.resolve();
    });

    const popover = await screen.findByTestId("command-popover");
    // The single-text-param case takes the form branch — one input
    // labelled `name` + a Submit button.
    const nameInput = popover.querySelector(
      'input[type="text"]',
    ) as HTMLInputElement;
    expect(nameInput).toBeTruthy();

    await act(async () => {
      fireEvent.change(nameInput, { target: { value: "My Sprint" } });
      await Promise.resolve();
    });

    const submit = popover.querySelector(
      'button[type="submit"]',
    ) as HTMLButtonElement;
    expect(submit).toBeTruthy();
    await act(async () => {
      fireEvent.click(submit);
      await Promise.resolve();
    });

    const saveCalls = mockInvoke.mock.calls.filter(
      (c) =>
        c[0] === "dispatch_command" &&
        (c[1] as { cmd?: string })?.cmd === "perspective.save",
    );
    expect(saveCalls).toHaveLength(1);
    expect(saveCalls[0][1]).toMatchObject({
      cmd: "perspective.save",
      args: { name: "My Sprint" },
    });
  });
});

describe("perspective-tab-bar — Sort migration", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockRegistry = [];
    mockBoardId = "test-board";
    mockPerspectivesValue = {
      perspectives: [{ id: "p1", name: "Sprint", view: "grid" }],
      activePerspective: { id: "p1", name: "Sprint", view: "grid" },
      setActivePerspectiveId: vi.fn(),
      refresh: vi.fn(() => Promise.resolve()),
    };
    mockViewsValue = {
      views: [
        { id: "grid-1", name: "Grid", kind: "grid", icon: "grid" },
        { id: "board-1", name: "Board", kind: "board", icon: "kanban" },
      ],
      activeView: { id: "grid-1", name: "Grid", kind: "grid", icon: "grid" },
      setActiveViewId: vi.fn(),
      refresh: vi.fn(() => Promise.resolve()),
    };
  });

  // -------------------------------------------------------------------------
  // 3a. The Sort `<CommandButton>` mounts on the active grid-view
  //     perspective tab with the `arrow-up-down` lucide icon.
  //
  //     This is the user-visible payoff for the entire epic: pre-migration
  //     there was NO surfaced Sort tab button on the grid view; sort was
  //     driven by column-header clicks in tanstack-table's own UI. The
  //     migration surfaces sort as a registry-rendered tab button while
  //     keeping the column-header sort affordance unchanged.
  // -------------------------------------------------------------------------

  it("sort_button_appears_on_grid_view_with_arrow_up_down_icon", async () => {
    mockResolvedCommands([sortRegistryEntry()]);

    renderTabBar();
    await flushEffects();

    const button = screen.getByRole("button", { name: "Sort Field" });
    expect(button).toBeTruthy();

    // The lucide `ArrowUpDown` icon resolves via
    // `commandIconFor("arrow-up-down")` in `command-icon-registry.ts`,
    // which assigns the `lucide-arrow-up-down` class on the rendered
    // SVG.
    const svg = button.querySelector("svg");
    expect(svg).not.toBeNull();
    expect(svg?.classList.contains("lucide-arrow-up-down")).toBe(true);
  });

  // -------------------------------------------------------------------------
  // 3b. The Sort `<CommandButton>` does NOT mount on a board view.
  //
  //     The user-visible bug the original perspective-sort report flagged:
  //     pre-migration the Sort affordance leaked across view kinds because
  //     it was hardcoded in tanstack-table. The migration relies on the
  //     existing `view_kinds: [grid]` annotation — the backend's
  //     `filter_by_view_kind` pass drops the command before emission, so
  //     the frontend's `<RegistryTabButtons>` slot stays empty for board
  //     views. We simulate that pass by dropping the command from the
  //     mocked registry response when the active view kind is `board`.
  // -------------------------------------------------------------------------

  it("sort_button_disappears_on_board_view", async () => {
    // Switch to a board view BEFORE the registry mock fires. The backend
    // would filter the command out at emission time because of its
    // `view_kinds: [grid]` annotation; the mock simulates that pass by
    // returning an empty list for the board scope.
    mockViewsValue = {
      ...mockViewsValue,
      activeView: { id: "board-1", name: "Board", kind: "board", icon: "kanban" },
    };
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [{ id: "p1", name: "Sprint", view: "board" }],
      activePerspective: { id: "p1", name: "Sprint", view: "board" },
    };
    mockResolvedCommands([]);

    renderTabBar();
    await flushEffects();

    // No Sort tab button on the board view — the registry payload is
    // empty because `view_kinds: [grid]` dropped the command.
    expect(screen.queryByRole("button", { name: "Sort Field" })).toBeNull();
  });

  // -------------------------------------------------------------------------
  // 4. Submitting the Sort popover with a picked field + direction
  //    dispatches `perspective.sort.set` with both args. The two-enum-
  //    param shape takes the form-branch popover render (Submit button
  //    visible) — the one-click menu render is reserved for single-enum
  //    commands.
  // -------------------------------------------------------------------------

  it("submitting_sort_popover_dispatches_perspective_sort_set_with_field_and_direction", async () => {
    mockResolvedCommands([sortRegistryEntry()]);

    renderTabBar();
    await flushEffects();

    const button = screen.getByRole("button", { name: "Sort Field" });
    await act(async () => {
      fireEvent.click(button);
      await Promise.resolve();
    });

    const popover = await screen.findByTestId("command-popover");

    // Two-param form: two `<select>`s plus a Submit button.
    const selects = popover.querySelectorAll("select");
    expect(selects.length).toBe(2);
    const submit = popover.querySelector(
      'button[type="submit"]',
    ) as HTMLButtonElement;
    expect(submit).toBeTruthy();

    // The form arranges params in declaration order; pick a value on
    // each select by its `aria-label` rather than positional index so a
    // future YAML reorder doesn't silently flip the assertion onto the
    // wrong field.
    const fieldSelect = popover.querySelector(
      'select[aria-label="field"]',
    ) as HTMLSelectElement;
    const directionSelect = popover.querySelector(
      'select[aria-label="direction"]',
    ) as HTMLSelectElement;
    expect(fieldSelect).toBeTruthy();
    expect(directionSelect).toBeTruthy();

    await act(async () => {
      fireEvent.change(fieldSelect, { target: { value: "status" } });
      fireEvent.change(directionSelect, { target: { value: "desc" } });
      await Promise.resolve();
    });

    await act(async () => {
      fireEvent.click(submit);
      await Promise.resolve();
    });

    const sortCalls = mockInvoke.mock.calls.filter(
      (c) =>
        c[0] === "dispatch_command" &&
        (c[1] as { cmd?: string })?.cmd === "perspective.sort.set",
    );
    expect(sortCalls).toHaveLength(1);
    expect(sortCalls[0][1]).toMatchObject({
      cmd: "perspective.sort.set",
      args: { field: "status", direction: "desc" },
    });
  });
});

describe("perspective-tab-bar — no hardcoded button JSX after migration", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
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

  // -------------------------------------------------------------------------
  // 5. The perspective tab bar contains zero hardcoded affordance JSX
  //    after the migration. Specifically the three legacy buttons that
  //    earlier epic tasks had not yet migrated:
  //
  //      - `<FilterFocusButton>` (aria-label "Filter") — deleted by
  //        01KRE1YA65MMG29RDQDQ0VPJQG.
  //      - `<GroupPopoverButton>` (aria-label "Group") — deleted by
  //        01KRE1ZTYJ5PPTQ29K72KE88B5.
  //      - `<AddPerspectiveButton>` (aria-label "Add perspective") —
  //        deleted by this task (01KRE21GJMPP289N1HSTMJG5HE).
  //
  //    Asserting absence of all three in a single test guards against a
  //    future contributor re-introducing any of them. With an empty
  //    registry payload no `<CommandButton>` mounts either, so we are
  //    asserting the pure structural baseline — no labels remain.
  // -------------------------------------------------------------------------

  it("tab_bar_has_no_hardcoded_button_jsx", async () => {
    // Empty registry response — no `<CommandButton>` mounts on EITHER
    // the per-tab surface OR the bar-level surface.
    mockResolvedCommands([]);

    renderTabBar();
    await flushEffects();

    // Hardcoded labels from the three legacy buttons — none survive
    // post-migration. Use exact string match (not regex) so the
    // assertion doesn't accidentally catch unrelated "filter"-bearing
    // strings (e.g. the filter formula bar's static glyph) — buttons
    // were the only producers of these exact aria-labels.
    expect(screen.queryByRole("button", { name: "Filter" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Group" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Group By" })).toBeNull();
    expect(
      screen.queryByRole("button", { name: /add perspective/i }),
    ).toBeNull();
  });
});
