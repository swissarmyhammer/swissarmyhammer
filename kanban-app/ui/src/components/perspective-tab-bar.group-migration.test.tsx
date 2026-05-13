/**
 * Regression tests for the Group tab-button migration to command-driven
 * rendering (task 01KRE1ZTYJ5PPTQ29K72KE88B5).
 *
 * Before the migration the active perspective tab rendered a hardcoded
 * `<GroupPopoverButton>` whose popover hosted `<GroupSelector>`. After
 * the migration the same affordance is a registry-rendered
 * `<CommandButton>` driven by the YAML-annotated `perspective.group`
 * command — and the popover is the generic `<CommandPopover>` with an
 * enum-shaped `<select>` populated by the backend
 * `PerspectiveFieldsResolver`.
 *
 * This is the FIRST migration to exercise the picker pipeline
 * end-to-end: enum param → backend-supplied options → frontend dropdown
 * → dispatch with picked value.
 *
 * Four contracts locked here:
 *
 *   1. The Group `<CommandButton>` carries the `group` lucide icon
 *      derived from `tab_button.icon`.
 *   2. Clicking the button opens a popover whose `<select>` is populated
 *      from `command.params[0].options` (backend resolver output).
 *   3. Submitting the popover with a picked field dispatches
 *      `perspective.group` with `{ group: <field-value>, perspective_id }`.
 *   4. The button carries the `text-primary` highlight when
 *      `perspective.group` is set on the active perspective.
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
// Domain context mocks — same shape as the Filter migration sibling so
// this file feels at home next to its peers.
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
 * Registry payload shape for the new `perspective.group` tab-button
 * command. Mirrors the YAML annotation introduced by this migration:
 * `tab_button: { icon: "group" }`, `params[0]` is enum-shaped with
 * options populated by `PerspectiveFieldsResolver`.
 */
function groupRegistryEntry(
  options: readonly { value: string; label: string }[] = [],
) {
  return {
    id: "perspective.group",
    name: "Group By",
    tab_button: { icon: "group" },
    params: [
      {
        name: "group",
        from: "args",
        shape: "enum",
        options_from: "perspective.fields",
        options,
      },
      { name: "perspective_id", from: "scope_chain" },
    ],
    keys: {},
  };
}

/** Install an `invoke` mock that returns `commands` for every `list_commands_for_scope` call. */
function mockResolvedCommands(commands: unknown[]) {
  mockInvoke.mockImplementation((cmd: string, _args?: unknown) => {
    if (cmd === "list_commands_for_scope") return Promise.resolve(commands);
    return Promise.resolve(null);
  });
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
 * `<RegistryTabButtons>` calls `invoke` inside a `useEffect` and writes
 * the result via `setCommands`. Three event-loop turns reliably cover
 * resolve → setState → register effect.
 */
async function flushEffects() {
  await act(async () => {
    for (let i = 0; i < 3; i += 1) {
      // eslint-disable-next-line no-await-in-loop
      await new Promise<void>((resolve) => setTimeout(resolve, 0));
    }
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("perspective-tab-bar — Group command migration", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
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
  // 1. The registry-rendered Group `<CommandButton>` mounts with the
  //    `group` lucide icon (resolved from `tab_button.icon`).
  // -------------------------------------------------------------------------

  it("group_command_button_renders_with_group_icon", async () => {
    mockResolvedCommands([groupRegistryEntry()]);

    renderTabBar();
    await flushEffects();

    // `<CommandButton>` derives its `aria-label` from `command.name`
    // ("Group By"). The legacy hardcoded `<GroupPopoverButton>`
    // (aria-label "Group") is deleted by this migration so the new
    // name is unambiguous.
    const button = screen.getByRole("button", { name: "Group By" });
    expect(button).toBeTruthy();

    // The lucide `Group` icon resolves via `commandIconFor("group")` in
    // `command-icon-registry.ts`. The icon-registry assigns it the
    // `lucide-group` class so we can pin the icon identity without
    // depending on internal SVG shape.
    const svg = button.querySelector("svg");
    expect(svg).not.toBeNull();
    expect(svg?.classList.contains("lucide-group")).toBe(true);
  });

  // -------------------------------------------------------------------------
  // 2. Clicking the button opens a popover whose menu items are populated
  //    from the backend-supplied `params[0].options`. The single-enum-
  //    param `perspective.group` command takes the one-click menu branch
  //    of `<CommandPopover>` — options render as buttons, not a `<select>`.
  // -------------------------------------------------------------------------

  it("group_popover_renders_field_options_from_command_emission", async () => {
    mockResolvedCommands([
      groupRegistryEntry([
        { value: "status", label: "Status" },
        { value: "assignee", label: "Assignee" },
      ]),
    ]);

    renderTabBar();
    await flushEffects();

    const button = screen.getByRole("button", { name: "Group By" });
    await act(async () => {
      fireEvent.click(button);
      await Promise.resolve();
    });

    // The popover content mounts when the button is clicked. The
    // `<CommandPopover>` body carries `data-testid="command-popover"`.
    const popover = await screen.findByTestId("command-popover");
    expect(popover).toBeTruthy();

    // The single-enum-param case renders one `<button>` per backend
    // option — picking IS the action, there is no native `<select>`.
    expect(popover.querySelector("select")).toBeNull();
    expect(popover.querySelector("button[type='submit']")).toBeNull();

    const optionLabels = Array.from(popover.querySelectorAll("button")).map(
      (b) => b.textContent,
    );
    expect(optionLabels).toContain("Status");
    expect(optionLabels).toContain("Assignee");
  });

  // -------------------------------------------------------------------------
  // 3. Picking a field in the popover dispatches `perspective.group` with
  //    the picked value plus the resolved perspective id. With the
  //    one-click menu pattern this is a single click on the option button
  //    — no Submit step.
  // -------------------------------------------------------------------------

  it("picking_a_group_field_dispatches_perspective_group_with_field_arg", async () => {
    mockResolvedCommands([
      groupRegistryEntry([
        { value: "status", label: "Status" },
        { value: "assignee", label: "Assignee" },
      ]),
    ]);

    renderTabBar();
    await flushEffects();

    const button = screen.getByRole("button", { name: "Group By" });
    await act(async () => {
      fireEvent.click(button);
      await Promise.resolve();
    });

    const popover = await screen.findByTestId("command-popover");

    // One click on the option button commits and dispatches. The
    // `<CommandButton>`'s `handleCommit` then dispatches the command
    // with the args bag.
    const statusButton = Array.from(popover.querySelectorAll("button")).find(
      (b) => b.textContent === "Status",
    ) as HTMLButtonElement;
    expect(statusButton).toBeTruthy();
    await act(async () => {
      fireEvent.click(statusButton);
      await Promise.resolve();
    });

    // Filter for the `perspective.group` dispatch specifically — the
    // popover-close path also dispatches `ui.setFocus` to restore focus,
    // and that is orthogonal to the picker contract this test pins.
    const dispatchCalls = mockInvoke.mock.calls.filter(
      (c) =>
        c[0] === "dispatch_command" &&
        (c[1] as { cmd?: string })?.cmd === "perspective.group",
    );
    expect(dispatchCalls).toHaveLength(1);
    expect(dispatchCalls[0][1]).toMatchObject({
      cmd: "perspective.group",
      args: { group: "status" },
    });
  });

  // -------------------------------------------------------------------------
  // 4. The Group `<CommandButton>` is highlighted (`text-primary`) when
  //    the active perspective has a `group` set.
  // -------------------------------------------------------------------------

  // -------------------------------------------------------------------------
  // 5. The popover surfaces a "(none)" affordance — first <option> in the
  //    enum select — when the `group` param carries `clear_command`.
  //    This restores the legacy `<GroupSelector>` "None" entry that the
  //    migration would otherwise drop (review-finding #4 on
  //    01KRE1ZTYJ5PPTQ29K72KE88B5).
  // -------------------------------------------------------------------------

  it("group_popover_renders_none_option_when_clear_command_present", async () => {
    mockResolvedCommands([
      {
        ...groupRegistryEntry([{ value: "status", label: "Status" }]),
        params: [
          {
            name: "group",
            from: "args",
            shape: "enum",
            options_from: "perspective.fields",
            options: [{ value: "status", label: "Status" }],
            clear_command: "perspective.clearGroup",
          },
          { name: "perspective_id", from: "scope_chain" },
        ],
      },
    ]);

    renderTabBar();
    await flushEffects();

    const button = screen.getByRole("button", { name: "Group By" });
    await act(async () => {
      fireEvent.click(button);
      await Promise.resolve();
    });

    const popover = await screen.findByTestId("command-popover");

    // The clear sentinel renders as the first menu item (button labelled
    // "(none)") when `clear_command` is set. The label tells the user
    // "this is a real pick that clears the state" — picking it is a
    // legitimate one-click action, not a "no selection yet" stub.
    const optionButtons = Array.from(popover.querySelectorAll("button"));
    expect(optionButtons.length).toBeGreaterThan(0);
    expect(optionButtons[0].textContent).toBe("(none)");

    // The single-enum-param menu branch renders no Submit affordance —
    // picking IS the action.
    expect(popover.querySelector("button[type='submit']")).toBeNull();
  });

  // -------------------------------------------------------------------------
  // 5b. Regression for task 01KRGW1DYD0T05PSTEDPT5D076 (iter-2 review):
  //     when BOTH `clear_command` is set AND `options` is non-empty, the
  //     select must render the (none) sentinel AND the real options
  //     alongside it — not the sentinel alone. Pre-iter-2 the user saw
  //     a popover with only "(none)" and no fields to group by; this
  //     test pins the "(none) plus the resolver-supplied options"
  //     contract end-to-end through the popover render pipeline.
  // -------------------------------------------------------------------------

  it("group_popover_renders_none_option_AND_real_options_when_both_present", async () => {
    mockResolvedCommands([
      {
        ...groupRegistryEntry(),
        params: [
          {
            name: "group",
            from: "args",
            shape: "enum",
            options_from: "perspective.fields",
            // The backend resolver supplied two real options; the
            // popover must render BOTH next to the (none) sentinel.
            options: [
              { value: "status", label: "Status" },
              { value: "assignees", label: "Assignees" },
            ],
            clear_command: "perspective.clearGroup",
          },
          { name: "perspective_id", from: "scope_chain" },
        ],
      },
    ]);

    renderTabBar();
    await flushEffects();

    const button = screen.getByRole("button", { name: "Group By" });
    await act(async () => {
      fireEvent.click(button);
      await Promise.resolve();
    });

    const popover = await screen.findByTestId("command-popover");

    // The full option set must be: (none) sentinel + every backend
    // option. Pre-iter-2 the regression would have rendered ONLY the
    // (none) sentinel when `clear_command` was set. With the one-click
    // menu pattern each entry is a button instead of an <option>.
    const optionLabels = Array.from(popover.querySelectorAll("button")).map(
      (b) => b.textContent,
    );
    expect(optionLabels).toEqual(["(none)", "Status", "Assignees"]);
  });

  // -------------------------------------------------------------------------
  // 6. Picking "(none)" in the popover and submitting dispatches
  //    `perspective.clearGroup` (NOT `perspective.group`) with the
  //    scope-resolved perspective id and no `group` arg.
  // -------------------------------------------------------------------------

  it("picking_none_in_group_popover_dispatches_perspective_clearGroup", async () => {
    mockResolvedCommands([
      {
        ...groupRegistryEntry([{ value: "status", label: "Status" }]),
        params: [
          {
            name: "group",
            from: "args",
            shape: "enum",
            options_from: "perspective.fields",
            options: [{ value: "status", label: "Status" }],
            clear_command: "perspective.clearGroup",
          },
          { name: "perspective_id", from: "scope_chain" },
        ],
      },
    ]);

    renderTabBar();
    await flushEffects();

    const button = screen.getByRole("button", { name: "Group By" });
    await act(async () => {
      fireEvent.click(button);
      await Promise.resolve();
    });

    const popover = await screen.findByTestId("command-popover");

    // With the one-click menu pattern, clicking the "(none)" button
    // commits `{ group: "" }` directly — `<CommandButton>`'s
    // `handleCommit` redirects the empty-string sentinel to
    // `clear_command`.
    const noneButton = Array.from(popover.querySelectorAll("button")).find(
      (b) => b.textContent === "(none)",
    ) as HTMLButtonElement;
    expect(noneButton).toBeTruthy();
    await act(async () => {
      fireEvent.click(noneButton);
      await Promise.resolve();
    });

    // The redirection MUST dispatch `perspective.clearGroup` and NOT
    // `perspective.group` — the user-visible "None in the popover"
    // affordance is meaningless if the parent command is still the
    // one that fires.
    const groupDispatches = mockInvoke.mock.calls.filter(
      (c) =>
        c[0] === "dispatch_command" &&
        (c[1] as { cmd?: string })?.cmd === "perspective.group",
    );
    expect(groupDispatches).toHaveLength(0);

    const clearDispatches = mockInvoke.mock.calls.filter(
      (c) =>
        c[0] === "dispatch_command" &&
        (c[1] as { cmd?: string })?.cmd === "perspective.clearGroup",
    );
    expect(clearDispatches).toHaveLength(1);

    // The redirected dispatch's args bag must NOT carry the empty-string
    // `group` sentinel — the clear command's contract is that it takes
    // no value for the redirected param.
    const redirectedArgs = (
      clearDispatches[0][1] as { args?: Record<string, unknown> }
    ).args;
    expect(redirectedArgs).not.toHaveProperty("group");
  });

  // -------------------------------------------------------------------------
  // 7. With no `clear_command`, the popover renders ONLY the real
  //    options — no "(none)" sentinel button. Guards against a future
  //    change that accidentally treats every single-enum popover as
  //    clearable.
  // -------------------------------------------------------------------------

  it("group_popover_omits_none_entry_when_no_clear_command", async () => {
    mockResolvedCommands([
      groupRegistryEntry([{ value: "status", label: "Status" }]),
    ]);

    renderTabBar();
    await flushEffects();

    const button = screen.getByRole("button", { name: "Group By" });
    await act(async () => {
      fireEvent.click(button);
      await Promise.resolve();
    });

    const popover = await screen.findByTestId("command-popover");
    const optionLabels = Array.from(popover.querySelectorAll("button")).map(
      (b) => b.textContent,
    );
    // Only the real option — no clear sentinel because `clear_command`
    // is not declared on the `group` param.
    expect(optionLabels).toEqual(["Status"]);
    expect(optionLabels).not.toContain("(none)");
  });

  it("group_button_is_active_when_perspective_has_a_group_set", async () => {
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [
        { id: "p1", name: "Sprint", view: "board", group: "status" },
      ],
      activePerspective: {
        id: "p1",
        name: "Sprint",
        view: "board",
        group: "status",
      },
    };
    mockResolvedCommands([groupRegistryEntry()]);

    renderTabBar();
    await flushEffects();

    const button = screen.getByRole("button", { name: "Group By" });
    // `<CommandButton>` applies `text-primary` whenever `isActive` is
    // true. The migration wires `isActive={Boolean(perspective.group)}`
    // through `isCommandActiveForPerspective`, so a non-empty group
    // must light up the icon.
    expect(button.className).toMatch(/text-primary/);
    const svg = button.querySelector("svg");
    expect(svg?.getAttribute("fill")).toBe("currentColor");
  });
});
