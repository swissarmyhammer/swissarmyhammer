/**
 * Toolbar navigation — React/dispatch boundary tests.
 *
 * Asserts that every interactive `<NavBar />` element registers its
 * `toolbar:*` moniker as a spatial entry and that the activation key on a
 * focused toolbar button dispatches the expected command — Space for
 * `ui.inspect` (matching the universal "inspect / peek" convention) and
 * Enter for `app.search` (which is an activation verb, not an inspect).
 * The h/l spatial walking between toolbar elements is covered by Rust
 * unit tests.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { userEvent } from "vitest/browser";
import { render } from "vitest-browser-react";

// -------------------------------------------------------------------------
// Tauri mocks — route every invoke through the boundary stub while
// preserving the rest of the API surface (Resource, SERIALIZE_TO_IPC_FN,
// etc. used by `window` and `webviewWindow`).
// -------------------------------------------------------------------------

vi.mock("@tauri-apps/api/core", async () => {
  const actual = await vi.importActual<typeof import("@tauri-apps/api/core")>(
    "@tauri-apps/api/core",
  );
  const { tauriCoreMock } = await import("./setup-tauri-stub");
  const { invoke } = tauriCoreMock();
  return { ...actual, invoke };
});
vi.mock("@tauri-apps/api/event", async () => {
  const actual = await vi.importActual<typeof import("@tauri-apps/api/event")>(
    "@tauri-apps/api/event",
  );
  const { tauriEventMock } = await import("./setup-tauri-stub");
  return { ...actual, ...tauriEventMock() };
});
vi.mock("@tauri-apps/api/window", async () => {
  const actual = await vi.importActual<typeof import("@tauri-apps/api/window")>(
    "@tauri-apps/api/window",
  );
  const { tauriWindowMock } = await import("./setup-tauri-stub");
  return { ...actual, ...tauriWindowMock() };
});
vi.mock("@tauri-apps/api/webviewWindow", async () => {
  const actual = await vi.importActual<
    typeof import("@tauri-apps/api/webviewWindow")
  >("@tauri-apps/api/webviewWindow");
  const { tauriWebviewWindowMock } = await import("./setup-tauri-stub");
  return { ...actual, ...tauriWebviewWindowMock() };
});
vi.mock("@tauri-apps/plugin-log", async () => {
  const { tauriPluginLogMock } = await import("./setup-tauri-stub");
  return tauriPluginLogMock();
});

// -------------------------------------------------------------------------
// Context mocks — fixture data for every provider the nav bar reads.
// -------------------------------------------------------------------------

type MockPerspective = {
  id: string;
  name: string;
  view: string;
  filter?: string;
  group?: string;
};

const fixturePerspectives: MockPerspective[] = [
  { id: "default", name: "Default", view: "board" },
  { id: "archive", name: "Archive", view: "board" },
];

vi.mock("@/lib/perspective-context", () => ({
  usePerspectives: () => ({
    perspectives: fixturePerspectives,
    activePerspective: fixturePerspectives[0],
    setActivePerspectiveId: vi.fn(),
    refresh: vi.fn(() => Promise.resolve()),
  }),
}));

vi.mock("@/lib/views-context", () => {
  const views = [
    { id: "board", name: "Board", kind: "board", icon: "kanban" },
    { id: "grid", name: "Grid", kind: "grid", icon: "table" },
  ];
  return {
    ViewsProvider: ({ children }: { children: React.ReactNode }) => children,
    useViews: () => ({
      views,
      activeView: views[0],
      setActiveViewId: vi.fn(),
      refresh: vi.fn(() => Promise.resolve()),
    }),
  };
});

const MOCK_PERCENT_FIELD_DEF = {
  name: "percent_complete",
  display_name: "% Complete",
  field_type: "PercentComplete",
};

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => ({
      entity: { name: "task", fields: [], search_display_field: "name" },
      fields: [],
    }),
    getFieldDef: (_entityType: string, fieldName: string) =>
      fieldName === "percent_complete" ? MOCK_PERCENT_FIELD_DEF : undefined,
    getEntityCommands: () => [],
    mentionableTypes: [],
    loading: false,
  }),
}));

vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => ({
    keymap_mode: "vim",
    scope_chain: [],
    open_boards: [],
    has_clipboard: false,
    clipboard_entity_type: null,
    windows: {},
    recent_boards: [],
  }),
  useUIStateLoading: () => ({
    state: {
      keymap_mode: "vim",
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

vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({ getEntities: () => [], getEntity: () => undefined }),
  useFieldValue: () => "Test Board",
}));

vi.mock("@/components/fields/field", () => ({
  Field: (props: Record<string, unknown>) => (
    <span data-testid="field-percent">{String(props.entityId)}</span>
  ),
}));

const FIXTURE_BOARD_DATA = {
  board: {
    entity_type: "board",
    id: "b1",
    moniker: "board:b1",
    fields: { name: { String: "Test Board" } },
  },
  columns: [],
  tags: [],
  virtualTagMeta: [],
  summary: {
    total_tasks: 5,
    total_actors: 2,
    ready_tasks: 3,
    blocked_tasks: 1,
    done_tasks: 1,
    percent_complete: 20,
  },
};

const FIXTURE_OPEN_BOARDS = [
  { path: "/boards/a/.kanban", name: "Board A", is_active: true },
];

vi.mock("@/components/window-container", () => ({
  useBoardData: () => FIXTURE_BOARD_DATA,
  useOpenBoards: () => FIXTURE_OPEN_BOARDS,
  useActiveBoardPath: () => "/boards/a/.kanban",
  useHandleSwitchBoard: () => vi.fn(),
}));

vi.mock("@/lib/command-scope", async () => {
  const actual = await vi.importActual<typeof import("@/lib/command-scope")>(
    "@/lib/command-scope",
  );
  return {
    ...actual,
    useCommandBusy: () => ({ isBusy: false }),
  };
});

// -------------------------------------------------------------------------
// Imports that see the mocks above.
// -------------------------------------------------------------------------

import { setupTauriStub, type TauriStubHandles } from "./setup-tauri-stub";
import {
  AppWithToolbarFixture,
  TOOLBAR_MONIKERS,
} from "./spatial-toolbar-fixture";

const POLL_TIMEOUT = 500;

describe("toolbar — React/dispatch boundary", () => {
  let handles: TauriStubHandles;

  beforeEach(() => {
    handles = setupTauriStub();
  });

  it("every toolbar element registers its moniker as a spatial entry", async () => {
    await render(<AppWithToolbarFixture />);

    for (const m of Object.values(TOOLBAR_MONIKERS)) {
      await expect
        .poll(
          () =>
            handles.invocations().some((i) => {
              if (i.cmd !== "spatial_register") return false;
              const a = i.args as { args?: { moniker?: string } };
              return a.args?.moniker === m;
            }),
          { timeout: POLL_TIMEOUT },
        )
        .toBe(true);
    }
  });

  it("Space on the inspect button dispatches ui.inspect with the board moniker", async () => {
    const screen = await render(<AppWithToolbarFixture />);
    const inspectEl = screen
      .getByTestId(`data-moniker:${TOOLBAR_MONIKERS.inspectBoard}`)
      .element() as HTMLElement;

    await userEvent.click(inspectEl);
    // Snapshot count so the Space assertion isolates only the keypress
    // dispatch (the click itself already dispatched ui.inspect).
    const beforeSpace = handles.dispatchedCommands().length;

    await userEvent.keyboard(" ");

    await expect
      .poll(() => handles.dispatchedCommands().length, {
        timeout: POLL_TIMEOUT,
      })
      .toBeGreaterThan(beforeSpace);

    const postSpace = handles.dispatchedCommands().slice(beforeSpace);
    expect(
      postSpace.some((d) => d.cmd === "ui.inspect" && d.target === "board:b1"),
    ).toBe(true);
  });

  it("Enter on the search button dispatches app.search", async () => {
    const screen = await render(<AppWithToolbarFixture />);
    const searchEl = screen
      .getByTestId(`data-moniker:${TOOLBAR_MONIKERS.search}`)
      .element() as HTMLElement;

    await userEvent.click(searchEl);
    const beforeEnter = handles.dispatchedCommands().length;

    await userEvent.keyboard("{Enter}");

    await expect
      .poll(() => handles.dispatchedCommands().length, {
        timeout: POLL_TIMEOUT,
      })
      .toBeGreaterThan(beforeEnter);

    const postEnter = handles.dispatchedCommands().slice(beforeEnter);
    expect(postEnter.some((d) => d.cmd === "app.search")).toBe(true);
  });

  it("pressing h from a focused toolbar element dispatches nav.left", async () => {
    const screen = await render(<AppWithToolbarFixture />);
    const searchEl = screen
      .getByTestId(`data-moniker:${TOOLBAR_MONIKERS.search}`)
      .element() as HTMLElement;

    await userEvent.click(searchEl);
    const before = handles.dispatchedCommands().length;
    await userEvent.keyboard("h");

    await expect
      .poll(
        () =>
          handles
            .dispatchedCommands()
            .slice(before)
            .some((d) => d.cmd === "nav.left"),
        { timeout: POLL_TIMEOUT },
      )
      .toBe(true);
  });
});
