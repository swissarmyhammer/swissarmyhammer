/**
 * Tests for the no-popup `+` (Add Perspective) flow on the perspective tab
 * bar (task 01KTYN8GB25ZFKSXWA0QA283PG).
 *
 * UX contract (owner directive — "popups and buttons, sheesh"):
 *
 *   1. Clicking `+` IMMEDIATELY dispatches `perspective.save` with a
 *      generated unique name (the first free "Untitled" / "Untitled N"
 *      slot among the perspectives visible in the active view scope — the
 *      same convention as the backend's `first_free_untitled_name`). No
 *      popover opens; there is no form to fill before the create.
 *   2. When the created perspective appears in the perspectives list (the
 *      store event loop round-trip), the tab activates and arms the
 *      existing inline rename machinery so the user can type the real
 *      name right away. The new entity is identified by the ID the save
 *      dispatch returned — never by name matching.
 *   3. Escape during that first rename KEEPS the generated name — the
 *      perspective was durably created on click; cancelling a rename
 *      never deletes an entity.
 *   4. A blank-named perspective renders a visible "Untitled" placeholder
 *      in its tab (muted/italic, matching the blank-value display
 *      convention in `fields/displays/*`) — never an invisible tab.
 *
 * Harness mirrors `perspective-tab-bar.add-and-sort-migration.test.tsx`
 * (registry-driven render via mocked `useCommandList`, real command-scope
 * dispatch through the mocked Tauri `invoke`).
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
// Domain context mocks
// ---------------------------------------------------------------------------

type MockPerspective = {
  id: string;
  name: string;
  view: string;
  view_id?: string;
  filter?: string;
  group?: string;
};

const mockSetActivePerspectiveId = vi.fn();
const mockRefresh = vi.fn(() => Promise.resolve());

let mockPerspectivesValue = {
  perspectives: [] as MockPerspective[],
  activePerspective: null as MockPerspective | null,
  setActivePerspectiveId: mockSetActivePerspectiveId,
  refresh: mockRefresh,
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

vi.mock("@/components/window-container", () => ({
  useBoardData: () => ({
    board: {
      entity_type: "board",
      id: "test-board",
      moniker: "board:test-board",
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

import {
  PerspectiveTabBar,
  generateUntitledName,
  BLANK_NAME_PLACEHOLDER,
} from "./perspective-tab-bar";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { FocusLayer } from "./focus-layer";
import { asSegment } from "@/types/spatial";

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/** Registry payload for the `perspective.save` (`+`) tab-button command. */
function addRegistryEntry() {
  return {
    id: "perspective.save",
    name: "Save Perspective",
    scope: [],
    context_menu: false,
    available: true,
    tab_button: { icon: "plus" },
    params: [
      { name: "name", from: "args", shape: "text" },
      { name: "view_id", from: "scope_chain", entity_type: "view" },
    ],
    keys: {},
  };
}

/** Render `<PerspectiveTabBar>` inside the standard provider stack. */
function renderTabBar() {
  // Fresh elements per call — reusing one JSX tree reference lets React
  // bail out on rerender (identical element props), which would hide the
  // mocked context value changes the arming tests rely on.
  const makeTree = () => (
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <TooltipProvider delayDuration={100}>
            <PerspectiveTabBar />
          </TooltipProvider>
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>
  );
  const result = render(makeTree());
  return { ...result, rerenderTabBar: () => result.rerender(makeTree()) };
}

/** Flush the registry effects + dispatch microtasks. */
async function flushEffects() {
  await act(async () => {
    for (let i = 0; i < 4; i += 1) {
      // eslint-disable-next-line no-await-in-loop
      await new Promise<void>((resolve) => setTimeout(resolve, 0));
    }
  });
}

/** All `dispatch_command` invoke payloads for the given command id. */
function dispatchCalls(cmd: string): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter(
      (c) =>
        c[0] === "dispatch_command" && (c[1] as { cmd?: string })?.cmd === cmd,
    )
    .map((c) => c[1] as Record<string, unknown>);
}

/** Click the `+` (Save Perspective) tab-bar button. */
async function clickAdd() {
  const button = screen.getByRole("button", { name: "Save Perspective" });
  await act(async () => {
    fireEvent.click(button);
    // Let the dispatch → unwrap-created-id → set-pending microtask chain
    // settle inside act so the pending-create state update is act-wrapped.
    for (let i = 0; i < 4; i += 1) {
      // eslint-disable-next-line no-await-in-loop
      await Promise.resolve();
    }
  });
}

/**
 * Mock the `perspective.save` dispatch result: the views `save perspective`
 * op returns `{ ok, perspective: { id, … }, entry_id }`, the plugin's
 * `perspective.save` unwraps it (`unwrapResult`, same precedent as
 * `perspective.list`), and the Tauri `dispatch_command` envelope nests the
 * command's return under `result`.
 */
function mockSaveReturning(id: string) {
  mockInvoke.mockImplementation((...args: unknown[]) => {
    if (
      args[0] === "dispatch_command" &&
      (args[1] as { cmd?: string } | undefined)?.cmd === "perspective.save"
    ) {
      return Promise.resolve({ result: { ok: true, perspective: { id } } });
    }
    return Promise.resolve(null);
  });
}

beforeEach(() => {
  mockInvoke.mockReset();
  mockInvoke.mockImplementation(() => Promise.resolve(null));
  mockSetActivePerspectiveId.mockReset();
  mockRefresh.mockReset();
  mockRefresh.mockImplementation(() => Promise.resolve());
  mockRegistry = [];
  mockPerspectivesValue = {
    perspectives: [{ id: "p1", name: "Sprint", view: "board" }],
    activePerspective: { id: "p1", name: "Sprint", view: "board" },
    setActivePerspectiveId: mockSetActivePerspectiveId,
    refresh: mockRefresh,
  };
  mockViewsValue = {
    views: [{ id: "board-1", name: "Board", kind: "board", icon: "kanban" }],
    activeView: { id: "board-1", name: "Board", kind: "board", icon: "kanban" },
    setActiveViewId: vi.fn(),
    refresh: vi.fn(() => Promise.resolve()),
  };
});

// ---------------------------------------------------------------------------
// 1. Click → immediate create, no popup
// ---------------------------------------------------------------------------

describe("perspective + button — immediate create, no popup", () => {
  it("clicking_add_dispatches_perspective_save_immediately_with_generated_name", async () => {
    mockRegistry = [addRegistryEntry()];

    renderTabBar();
    await flushEffects();

    await clickAdd();

    // No popover anywhere — the popup path for `+` is deleted.
    expect(
      document.querySelector('[data-testid="command-popover"]'),
    ).toBeNull();

    // The create dispatched immediately through the command service with
    // the generated name and the active view's kind + instance id.
    const saveCalls = dispatchCalls("perspective.save");
    expect(saveCalls).toHaveLength(1);
    expect(saveCalls[0]).toMatchObject({
      cmd: "perspective.save",
      args: { name: "Untitled", view: "board", view_id: "board-1" },
    });
  });

  it("generated_name_dedupes_against_visible_untitled_perspectives", async () => {
    mockRegistry = [addRegistryEntry()];
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [
        { id: "p1", name: "Sprint", view: "board" },
        { id: "u1", name: "Untitled", view: "board" },
      ],
    };

    renderTabBar();
    await flushEffects();

    await clickAdd();

    const saveCalls = dispatchCalls("perspective.save");
    expect(saveCalls).toHaveLength(1);
    expect(saveCalls[0]).toMatchObject({
      args: { name: "Untitled 2" },
    });
  });

  it("dedupe_only_counts_perspectives_visible_in_the_active_view_scope", async () => {
    mockRegistry = [addRegistryEntry()];
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [
        { id: "p1", name: "Sprint", view: "board" },
        // Grid-kind "Untitled" is NOT visible on the active board view —
        // it must not bump the generated name.
        { id: "u1", name: "Untitled", view: "grid" },
        // Pinned to a different view instance — also invisible here.
        { id: "u2", name: "Untitled", view: "board", view_id: "board-2" },
      ],
    };

    renderTabBar();
    await flushEffects();

    await clickAdd();

    const saveCalls = dispatchCalls("perspective.save");
    expect(saveCalls).toHaveLength(1);
    expect(saveCalls[0]).toMatchObject({ args: { name: "Untitled" } });
  });
});

// ---------------------------------------------------------------------------
// 2. Rename arms when the created perspective appears (event-loop driven)
// ---------------------------------------------------------------------------

/**
 * Drive the full create flow: click `+` (with the save dispatch returning
 * the created id), then simulate the store event loop landing by appending
 * the created perspective to the mocked perspectives list and re-rendering.
 */
async function createAndLandPerspective(
  result: ReturnType<typeof renderTabBar>,
) {
  mockSaveReturning("p-new");
  await clickAdd();

  // Simulate the create→event→refetch round-trip: the created perspective
  // appears in the visible list.
  mockPerspectivesValue = {
    ...mockPerspectivesValue,
    perspectives: [
      ...mockPerspectivesValue.perspectives,
      { id: "p-new", name: "Untitled", view: "board", view_id: "board-1" },
    ],
  };
  await act(async () => {
    result.rerenderTabBar();
    await Promise.resolve();
  });
  await flushEffects();
}

describe("perspective + button — rename arms when the entity appears", () => {
  it("activates_and_arms_inline_rename_on_the_created_perspective", async () => {
    mockRegistry = [addRegistryEntry()];
    mockSaveReturning("p-new");

    const result = renderTabBar();
    await flushEffects();

    await clickAdd();

    // The entity has not appeared yet — the only CM6 editor is the filter
    // formula bar's; no tab hosts a rename editor (do not race the store).
    expect(result.container.querySelectorAll("button .cm-editor")).toHaveLength(
      0,
    );

    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [
        ...mockPerspectivesValue.perspectives,
        { id: "p-new", name: "Untitled", view: "board", view_id: "board-1" },
      ],
    };
    await act(async () => {
      result.rerenderTabBar();
      await Promise.resolve();
    });
    await flushEffects();

    // The new tab activates (perspective.switch via setActivePerspectiveId)…
    expect(mockSetActivePerspectiveId).toHaveBeenCalledWith("p-new");

    // …and arms the EXISTING inline rename machinery: the rename CM6
    // editor mounts inside the new tab's button, pre-filled with the
    // generated name.
    const renameEditor = result.container.querySelector("button .cm-editor");
    expect(
      renameEditor,
      "rename editor must mount in the new tab",
    ).toBeTruthy();
    expect(renameEditor?.querySelector(".cm-content")?.textContent).toContain(
      "Untitled",
    );
  });

  it("does_not_arm_rename_for_unrelated_list_changes", async () => {
    mockRegistry = [addRegistryEntry()];

    const result = renderTabBar();
    await flushEffects();

    // No `+` click — a perspective arriving from elsewhere (another
    // window, undo, …) must NOT arm rename.
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [
        ...mockPerspectivesValue.perspectives,
        { id: "p-other", name: "Untitled", view: "board" },
      ],
    };
    await act(async () => {
      result.rerenderTabBar();
      await Promise.resolve();
    });
    await flushEffects();

    expect(result.container.querySelectorAll("button .cm-editor")).toHaveLength(
      0,
    );
    expect(mockSetActivePerspectiveId).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// 3. Escape during the first rename keeps the generated name
// ---------------------------------------------------------------------------

describe("perspective + button — Escape during the first rename", () => {
  it("escape_keeps_the_generated_name_and_deletes_nothing", async () => {
    mockRegistry = [addRegistryEntry()];

    const result = renderTabBar();
    await flushEffects();
    await createAndLandPerspective(result);

    const cmContent = result.container.querySelector(
      "button .cm-editor .cm-content",
    ) as HTMLElement;
    expect(cmContent, "rename editor must be armed before Escape").toBeTruthy();

    mockInvoke.mockClear();

    // Escape in CUA mode cancels the EDIT only.
    await act(async () => {
      cmContent.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "Escape",
          bubbles: true,
          cancelable: true,
        }),
      );
      await new Promise((r) => setTimeout(r, 50));
    });

    // The rename editor closes; the tab keeps showing the generated name.
    expect(result.container.querySelector("button .cm-editor")).toBeNull();
    expect(screen.getByText("Untitled")).toBeTruthy();

    // Pinned semantics: the perspective survives — no delete, no rename.
    expect(dispatchCalls("perspective.delete")).toHaveLength(0);
    expect(dispatchCalls("perspective.rename")).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// 4. Blank-name placeholder (presentation only)
// ---------------------------------------------------------------------------

describe("perspective tab — blank-name placeholder", () => {
  it("blank_name_renders_a_visible_untitled_placeholder", async () => {
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [
        { id: "p-blank", name: "", view: "board" },
        { id: "p1", name: "Sprint", view: "board" },
      ],
      activePerspective: { id: "p1", name: "Sprint", view: "board" },
    };

    renderTabBar();
    await flushEffects();

    // The blank-named tab renders visible placeholder text styled per the
    // app's blank-value display convention (muted + italic — see
    // `fields/displays/markdown-display.tsx`), never an invisible tab.
    const placeholder = screen.getByText("Untitled");
    expect(placeholder).toBeTruthy();
    expect(placeholder.className).toContain("text-muted-foreground");
    expect(placeholder.className).toContain("italic");

    // The non-blank sibling renders its stored name as plain text.
    const named = screen.getByText("Sprint");
    expect(named).toBeTruthy();
    expect(named.className ?? "").not.toContain("italic");
  });

  it("whitespace_only_name_renders_the_placeholder_too", async () => {
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [{ id: "p-ws", name: "   ", view: "board" }],
      activePerspective: { id: "p-ws", name: "   ", view: "board" },
    };

    renderTabBar();
    await flushEffects();

    const placeholder = screen.getByText("Untitled");
    expect(placeholder).toBeTruthy();
    expect(placeholder.className).toContain("italic");
  });
});

// ---------------------------------------------------------------------------
// 5. Arm-by-id — name collisions never arm rename on a pre-existing tab
//    (review blocker B1 on card 01KTYN8GB25ZFKSXWA0QA283PG)
// ---------------------------------------------------------------------------

describe("perspective + button — arm by id, never by name", () => {
  it("colliding_visible_names_never_arm_rename_on_a_preexisting_tab", async () => {
    mockRegistry = [addRegistryEntry()];
    // The gap shape: "Untitled" was deleted, only "Untitled 2" remains
    // visible. A count-based generator would re-mint "Untitled 2" (a
    // collision) and a name-matching watcher would then arm rename on the
    // PRE-EXISTING tab instead of the created one.
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [{ id: "u2", name: "Untitled 2", view: "board" }],
      activePerspective: { id: "u2", name: "Untitled 2", view: "board" },
    };
    mockSaveReturning("p-new");

    const result = renderTabBar();
    await flushEffects();

    await clickAdd();

    // The generated name takes the first FREE slot — never a name that
    // already exists among the visible perspectives.
    const saveCalls = dispatchCalls("perspective.save");
    expect(saveCalls).toHaveLength(1);
    const sentName = (saveCalls[0].args as { name?: string })?.name;
    expect(
      sentName,
      "the generated name must not collide with a visible perspective",
    ).toBe("Untitled");

    // Before the created entity lands, nothing arms — in particular the
    // pre-existing "Untitled 2" tab must not activate or host a rename
    // editor.
    await flushEffects();
    expect(mockSetActivePerspectiveId).not.toHaveBeenCalled();
    expect(result.container.querySelectorAll("button .cm-editor")).toHaveLength(
      0,
    );

    // The created perspective lands (identified by the id the save
    // dispatch returned) → the NEW tab activates and arms rename.
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [
        ...mockPerspectivesValue.perspectives,
        { id: "p-new", name: "Untitled", view: "board", view_id: "board-1" },
      ],
    };
    await act(async () => {
      result.rerenderTabBar();
      await Promise.resolve();
    });
    await flushEffects();

    expect(mockSetActivePerspectiveId).toHaveBeenCalledWith("p-new");
    expect(mockSetActivePerspectiveId).not.toHaveBeenCalledWith("u2");
    expect(
      result.container.querySelector("button .cm-editor"),
      "rename editor must mount in the NEW tab",
    ).toBeTruthy();
  });

  it("pending_create_is_abandoned_when_the_view_switches_before_the_entity_lands", async () => {
    mockRegistry = [addRegistryEntry()];
    mockSaveReturning("p-new");

    const result = renderTabBar();
    await flushEffects();

    await clickAdd();
    expect(dispatchCalls("perspective.save")).toHaveLength(1);

    // The user switches views before the created perspective lands…
    mockViewsValue = {
      ...mockViewsValue,
      activeView: { id: "grid-1", name: "Grid", kind: "grid", icon: "table" },
    };
    await act(async () => {
      result.rerenderTabBar();
      await Promise.resolve();
    });
    await flushEffects();

    // …then switches back, and the created perspective becomes visible.
    // The stale create must NOT activate the tab or arm rename — the
    // pending state was bound to the dispatch-time view and abandoned on
    // the switch.
    mockViewsValue = {
      ...mockViewsValue,
      activeView: {
        id: "board-1",
        name: "Board",
        kind: "board",
        icon: "kanban",
      },
    };
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [
        ...mockPerspectivesValue.perspectives,
        { id: "p-new", name: "Untitled", view: "board", view_id: "board-1" },
      ],
    };
    await act(async () => {
      result.rerenderTabBar();
      await Promise.resolve();
    });
    await flushEffects();

    expect(mockSetActivePerspectiveId).not.toHaveBeenCalled();
    expect(result.container.querySelectorAll("button .cm-editor")).toHaveLength(
      0,
    );
  });
});

// ---------------------------------------------------------------------------
// 6. Frontend/backend drift pin for the generated-name convention
// ---------------------------------------------------------------------------

describe("generateUntitledName — frontend/backend drift pin", () => {
  // Lockstep table — mirrored verbatim by the Rust side's
  // `first_free_untitled_name_matches_the_frontend_generator` in
  // `crates/swissarmyhammer-kanban/src/commands/perspective_commands.rs`.
  // The two generators are cross-language mirrors of ONE convention: scan
  // for the first free "Untitled" / "Untitled N" slot by EXACT name match.
  // If either side changes, both tables must change together.
  const DRIFT_TABLE: ReadonlyArray<readonly [string[], string]> = [
    [[], "Untitled"],
    [["Untitled"], "Untitled 2"],
    [["Untitled", "Untitled 2"], "Untitled 3"],
    // Gap shape: the first free slot is reused, never a colliding re-mint.
    [["Untitled 2"], "Untitled"],
    // Prefix-only names are user names, not generated slots — no collision.
    [["Untitled tasks"], "Untitled"],
    [["Sprint", "Untitled", "Untitled 3"], "Untitled 2"],
  ];

  it("generates_the_first_free_untitled_slot", () => {
    for (const [names, expected] of DRIFT_TABLE) {
      expect(
        generateUntitledName(names.map((name) => ({ name }))),
        `taken=${JSON.stringify(names)}`,
      ).toBe(expected);
    }
  });

  it("blank_name_placeholder_literal_matches_the_rust_caption_placeholder", () => {
    // Literal drift pin: `BLANK_PERSPECTIVE_NAME_PLACEHOLDER` in
    // `crates/swissarmyhammer-kanban/src/scope_commands.rs` pins the same
    // string for the "Go to Perspective: …" palette caption — change both
    // or neither.
    expect(BLANK_NAME_PLACEHOLDER).toBe("Untitled");
  });
});
