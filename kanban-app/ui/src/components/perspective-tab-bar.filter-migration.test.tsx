/**
 * Regression tests for the Filter tab-button migration to command-driven
 * rendering (task 01KRE1YA65MMG29RDQDQ0VPJQG).
 *
 * Before the migration the active perspective tab rendered a hardcoded
 * `<FilterFocusButton>` whose click called a local `onFocus` callback to
 * focus the formula bar. After the migration the same affordance is a
 * registry-rendered `<CommandButton>` driven by the new no-arg
 * `perspective.filter.focus` command:
 *
 *   - Click dispatches `perspective.filter.focus` (no popover; every
 *     param resolves from scope) with the active perspective id.
 *   - The backend's `FocusFilterCmd` returns a `FocusFilter` marker
 *     envelope the dispatcher converts into a `ui.focus.filter` Tauri
 *     event the formula bar subscribes to. (The backend side is exercised
 *     by `swissarmyhammer-kanban`'s `focus_filter_command_*` Rust tests;
 *     this file pins only the React side of the wire.)
 *
 * Five contracts locked here:
 *
 *   1. Clicking the Filter `<CommandButton>` dispatches
 *      `perspective.filter.focus` with the host perspective's id.
 *   2. The button carries the `text-primary` highlight when
 *      `perspective.filter` is set.
 *   3. The button does NOT carry the highlight when
 *      `perspective.filter` is unset/empty.
 *   4. The button registers the new spatial-nav moniker
 *      `perspective_tab.perspective.filter.focus:{id}` — the legacy
 *      `perspective_tab.filter:{id}` is gone.
 *   5. `<FilterEditorBody>` calls `focus()` on its CM6 editor when a
 *      `ui.focus.filter` event arrives carrying its perspective id, and
 *      ignores events targeted at a sibling perspective.
 */

import type React from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act, fireEvent } from "@testing-library/react";
import { TooltipProvider } from "@/components/ui/tooltip";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before any module imports that pull
// command-scope / filter-editor / perspective-tab-bar.
// ---------------------------------------------------------------------------

type ListenCallback = (event: { payload: unknown }) => void;

const { mockInvoke, mockListen, listeners } = vi.hoisted(() => {
  const listeners = new Map<string, ListenCallback[]>();
  const mockInvoke = vi.fn(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (..._args: any[]): Promise<unknown> => Promise.resolve(null),
  );
  const mockListen = vi.fn(
    (eventName: string, cb: ListenCallback): Promise<() => void> => {
      const cbs = listeners.get(eventName) ?? [];
      cbs.push(cb);
      listeners.set(eventName, cbs);
      return Promise.resolve(() => {
        const arr = listeners.get(eventName);
        if (arr) {
          const idx = arr.indexOf(cb);
          if (idx >= 0) arr.splice(idx, 1);
        }
      });
    },
  );
  return { mockInvoke, mockListen, listeners };
});

vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (...a: Parameters<typeof mockListen>) => mockListen(...a),
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
// Domain context mocks — same shape as the existing perspective-tab-bar
// test files so this file feels at home next to its siblings.
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
import { FilterEditor } from "./filter-editor";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { FocusLayer } from "./focus-layer";
import { asSegment } from "@/types/spatial";
import { CommandScopeProvider } from "@/lib/command-scope";

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/** Install an `invoke` mock that returns `commands` for every `list_commands_for_scope` call. */
function mockResolvedCommands(
  commands: Array<{
    id: string;
    name: string;
    tab_button?: { icon: string };
    params?: readonly { name: string; from?: string }[];
    keys?: Record<string, string> | Record<string, never>;
  }>,
) {
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

/** Build a registry payload for the new `perspective.filter.focus` command. */
function focusFilterRegistryEntry() {
  return {
    id: "perspective.filter.focus",
    name: "Focus Filter",
    tab_button: { icon: "filter" },
    // `params` is intentionally `scope_chain`-only: clicking dispatches
    // immediately, no popover. This mirrors the YAML in
    // `swissarmyhammer-kanban/builtin/commands/perspective.yaml`.
    params: [{ name: "perspective_id", from: "scope_chain" }],
    keys: {},
  };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("perspective-tab-bar — Filter command migration", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockListen.mockClear();
    listeners.clear();
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
  // 1. Clicking the registry-rendered Filter button dispatches
  //    `perspective.filter.focus` with the host perspective id.
  // -------------------------------------------------------------------------

  it("filter_command_button_dispatches_perspective_filter_focus_on_click", async () => {
    mockResolvedCommands([focusFilterRegistryEntry()]);

    renderTabBar();
    await flushEffects();

    // The registry-rendered button carries `aria-label="Focus Filter"`
    // (derived from `command.name`). The hardcoded `<FilterFocusButton>`
    // (aria-label "Filter") is deleted, so name disambiguation is
    // unambiguous — `getByRole` won't collide.
    const button = screen.getByRole("button", { name: "Focus Filter" });

    await act(async () => {
      fireEvent.click(button);
      await Promise.resolve();
    });

    // Expect exactly one `dispatch_command` call for the focus command.
    const dispatchCalls = mockInvoke.mock.calls.filter(
      (c) => c[0] === "dispatch_command",
    );
    expect(dispatchCalls).toHaveLength(1);
    expect(dispatchCalls[0][1]).toMatchObject({
      cmd: "perspective.filter.focus",
    });
  });

  // -------------------------------------------------------------------------
  // 2. The Filter `<CommandButton>` is highlighted (`text-primary`) when
  //    the active perspective has a non-empty filter.
  // -------------------------------------------------------------------------

  it("filter_button_is_active_when_perspective_has_a_filter", async () => {
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [
        { id: "p1", name: "Sprint", view: "board", filter: "#bug" },
      ],
      activePerspective: {
        id: "p1",
        name: "Sprint",
        view: "board",
        filter: "#bug",
      },
    };
    mockResolvedCommands([focusFilterRegistryEntry()]);

    renderTabBar();
    await flushEffects();

    const button = screen.getByRole("button", { name: "Focus Filter" });
    // `<CommandButton>` applies `text-primary` whenever `isActive` is
    // true. The migration wires `isActive={Boolean(perspective.filter)}`
    // through `isCommandActiveForPerspective`, so a non-empty filter
    // must light up the icon.
    expect(button.className).toMatch(/text-primary/);
    const svg = button.querySelector("svg");
    expect(svg?.getAttribute("fill")).toBe("currentColor");
  });

  // -------------------------------------------------------------------------
  // 3. The Filter `<CommandButton>` is NOT highlighted when the active
  //    perspective has no filter.
  // -------------------------------------------------------------------------

  it("filter_button_is_inactive_when_perspective_filter_is_undefined", async () => {
    // Default perspective has no `filter` — the beforeEach sets it to
    // `{ id: "p1", name: "Sprint", view: "board" }` with no filter
    // field, so this test relies on the default fixture.
    mockResolvedCommands([focusFilterRegistryEntry()]);

    renderTabBar();
    await flushEffects();

    const button = screen.getByRole("button", { name: "Focus Filter" });
    // Counterpoint to test #2 — the icon must NOT carry the primary
    // highlight when the filter is unset, and the SVG's `fill` must be
    // `none` so the icon renders as a muted glyph.
    expect(button.className).not.toMatch(/text-primary/);
    const svg = button.querySelector("svg");
    expect(svg?.getAttribute("fill")).toBe("none");
  });

  // -------------------------------------------------------------------------
  // 4. The new button uses the registry-derived moniker, NOT the legacy
  //    `perspective_tab.filter:{id}`.
  // -------------------------------------------------------------------------

  it("filter_button_uses_perspective_filter_focus_moniker", async () => {
    mockResolvedCommands([focusFilterRegistryEntry()]);

    const { container } = renderTabBar();
    await flushEffects();

    // The `<CommandButton>` registers its leaf moniker as
    // `${surface}.${command.id}:${surfaceId}` — for the perspective
    // tab bar that's `perspective_tab.perspective.filter.focus:p1`.
    // The DOM mirrors this via `data-segment` on the `<FocusScope>`
    // wrapper Pressable mounts internally.
    const newLeaf = container.querySelector(
      "[data-segment='perspective_tab.perspective.filter.focus:p1']",
    );
    expect(
      newLeaf,
      "the registry-driven `<CommandButton>` must register the new spatial-nav moniker",
    ).not.toBeNull();

    // The legacy moniker (from the deleted `<FilterFocusButton>`)
    // MUST be gone — if a future regression resurrects the hardcoded
    // button this assertion fires.
    const legacyLeaf = container.querySelector(
      "[data-segment='perspective_tab.filter:p1']",
    );
    expect(
      legacyLeaf,
      "the legacy perspective_tab.filter:{id} moniker must be deleted with <FilterFocusButton>",
    ).toBeNull();
  });

  // -------------------------------------------------------------------------
  // 5. `<FilterEditorBody>` reacts to `ui.focus.filter` events targeted
  //    at its perspective id, and ignores broadcasts for siblings.
  //
  // Mounted as a standalone `<FilterEditor>` (instead of through the
  // tab bar) so the test can intercept the inner CM6 editor's focus
  // call directly — the formula bar's own click-to-focus path is not
  // exercised here. The contract pinned: subscribe → match id →
  // imperative focus.
  // -------------------------------------------------------------------------

  it("filter_editor_focuses_on_ui_focus_filter_event_for_matching_perspective", async () => {
    const ref = { current: null as HTMLElement | null };
    function Capture({ children }: { children: React.ReactNode }) {
      return (
        <div ref={(el) => (ref.current = el)} data-testid="capture">
          {children}
        </div>
      );
    }

    render(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <EntityFocusProvider>
            <Capture>
              <CommandScopeProvider moniker="perspective:p1">
                <FilterEditor filter="" perspectiveId="p1" />
              </CommandScopeProvider>
            </Capture>
          </EntityFocusProvider>
        </FocusLayer>
      </SpatialFocusProvider>,
    );

    // Wait for `listen("ui.focus.filter", …)` inside the editor to
    // resolve so the listener is actually wired up before we fire.
    await act(async () => {
      for (let i = 0; i < 3; i += 1) {
        // eslint-disable-next-line no-await-in-loop
        await new Promise<void>((resolve) => setTimeout(resolve, 0));
      }
    });

    // Confirm the subscription registered against the expected channel.
    const registered = listeners.get("ui.focus.filter") ?? [];
    expect(
      registered.length,
      "FilterEditorBody must subscribe to ui.focus.filter",
    ).toBeGreaterThan(0);

    // Spy on the inner CM6 contenteditable's `focus()` so we can assert
    // the event triggered an imperative focus. The contenteditable is
    // the actual DOM target the editor's `innerRef.current?.focus()`
    // resolves to.
    const cmContent = ref.current?.querySelector(
      ".cm-content",
    ) as HTMLElement | null;
    expect(cmContent, ".cm-content must be present").not.toBeNull();
    const focusSpy = vi.spyOn(cmContent!, "focus");

    // Sibling-perspective broadcast — must NOT focus this editor.
    await act(async () => {
      for (const cb of registered) {
        cb({ payload: { perspective_id: "p2" } });
      }
      await Promise.resolve();
    });
    expect(
      focusSpy,
      "broadcast for a sibling perspective must not steal focus",
    ).not.toHaveBeenCalled();

    // Matching broadcast — must focus this editor.
    await act(async () => {
      for (const cb of registered) {
        cb({ payload: { perspective_id: "p1" } });
      }
      await Promise.resolve();
    });
    expect(
      focusSpy,
      "broadcast for this editor's perspective must call focus() on the CM6 content",
    ).toHaveBeenCalled();

    focusSpy.mockRestore();
  });
});
