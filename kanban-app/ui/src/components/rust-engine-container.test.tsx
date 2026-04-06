import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act, waitFor } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Mock Tauri APIs before importing components that use them.
// vi.hoisted runs before vi.mock hoisting so the references are valid.
// ---------------------------------------------------------------------------

type ListenCallback = (event: { payload: unknown }) => void;

const { mockInvoke, mockListen, listeners } = vi.hoisted(() => {
  const listeners = new Map<string, ListenCallback[]>();
  const mockInvoke = vi.fn((cmd: string) => {
    if (cmd === "get_ui_state")
      return Promise.resolve({
        palette_open: false,
        palette_mode: "command",
        keymap_mode: "cua",
        scope_chain: [],
        open_boards: [],
        windows: {},
        recent_boards: [],
      });
    if (cmd === "list_schemas") return Promise.resolve([]);
    return Promise.resolve(null);
  });
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
  invoke: mockInvoke,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: mockListen,
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));

// Import after mocks
import { useEntityStore } from "@/lib/entity-store-context";
import { useSchema } from "@/lib/schema-context";
import { useUIState } from "@/lib/ui-state-context";
import { useEntityFocus } from "@/lib/entity-focus-context";
import {
  RustEngineContainer,
  useRefreshEntities,
  useEntitiesByType,
} from "./rust-engine-container";

// ---------------------------------------------------------------------------
// Helper: emits a Tauri event to registered listeners
// ---------------------------------------------------------------------------
function emitTauriEvent(eventName: string, payload: unknown) {
  const cbs = listeners.get(eventName) ?? [];
  for (const cb of cbs) {
    cb({ payload });
  }
}

// ---------------------------------------------------------------------------
// Probe components that verify contexts are available
// ---------------------------------------------------------------------------

/** Renders "schema-ok" if SchemaProvider is present. */
function SchemaProbe() {
  useSchema();
  return <span data-testid="schema-ok">schema-ok</span>;
}

/** Renders "entity-store-ok" and task count. Uses reactive context for re-renders. */
function EntityStoreProbe() {
  // useEntityStore verifies provider exists; useEntitiesByType for reactive updates
  useEntityStore();
  const entitiesByType = useEntitiesByType();
  const tasks = entitiesByType.task ?? [];
  return (
    <span data-testid="entity-store-ok">entity-store-ok:{tasks.length}</span>
  );
}

/** Renders "entity-focus-ok" if EntityFocusProvider is present. */
function EntityFocusProbe() {
  useEntityFocus();
  return <span data-testid="entity-focus-ok">entity-focus-ok</span>;
}

/** Renders "ui-state-ok" if UIStateProvider is present. */
function UIStateProbe() {
  useUIState();
  return <span data-testid="ui-state-ok">ui-state-ok</span>;
}

/** Probe for refreshEntities context. */
function RefreshProbe() {
  const refreshEntities = useRefreshEntities();
  return (
    <button
      data-testid="refresh-btn"
      onClick={() => refreshEntities("/some/board")}
    >
      refresh
    </button>
  );
}

/** Combined probe component. */
function AllProbes() {
  return (
    <>
      <SchemaProbe />
      <EntityStoreProbe />
      <EntityFocusProbe />
      <UIStateProbe />
      <RefreshProbe />
    </>
  );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("RustEngineContainer", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listeners.clear();
  });

  it("provides all required contexts to children", async () => {
    await act(async () => {
      render(
        <RustEngineContainer>
          <AllProbes />
        </RustEngineContainer>,
      );
    });

    expect(screen.getByTestId("schema-ok")).toBeTruthy();
    expect(screen.getByTestId("entity-store-ok")).toBeTruthy();
    expect(screen.getByTestId("entity-focus-ok")).toBeTruthy();
    expect(screen.getByTestId("ui-state-ok")).toBeTruthy();
  });

  it("exposes refreshEntities via context", async () => {
    await act(async () => {
      render(
        <RustEngineContainer>
          <RefreshProbe />
        </RustEngineContainer>,
      );
    });

    expect(screen.getByTestId("refresh-btn")).toBeTruthy();
  });

  it("entity-created with populated fields adds entity without get_entity call", async () => {
    // When the watcher provides fields in the event payload, the handler
    // should use them directly — no IPC round-trip via get_entity.
    let getEntityCalled = false;
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_entity") {
        getEntityCalled = true;
        return Promise.resolve({ entity_type: "task", id: "t1", title: "X" });
      }
      if (cmd === "get_ui_state")
        return Promise.resolve({
          palette_open: false,
          palette_mode: "command",
          keymap_mode: "cua",
          scope_chain: [],
          open_boards: [],
          windows: {},
          recent_boards: [],
        });
      if (cmd === "list_schemas") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    await act(async () => {
      render(
        <RustEngineContainer>
          <EntityStoreProbe />
        </RustEngineContainer>,
      );
    });

    expect(screen.getByTestId("entity-store-ok").textContent).toBe(
      "entity-store-ok:0",
    );

    // Emit entity-created with populated fields — fast path
    await act(async () => {
      emitTauriEvent("entity-created", {
        kind: "entity-created",
        entity_type: "task",
        id: "t1",
        fields: { title: "New Task" },
      });
    });

    await waitFor(() => {
      expect(screen.getByTestId("entity-store-ok").textContent).toBe(
        "entity-store-ok:1",
      );
    });

    // get_entity should NOT have been called — fields came from event payload
    expect(getEntityCalled).toBe(false);
  });

  it("entity-created with empty fields falls back to get_entity", async () => {
    // When fields are empty (store event before watcher cached), fall back
    // to get_entity to fetch the full entity.
    const taskBag = { entity_type: "task", id: "t1", title: "Fetched" };
    let getEntityCalled = false;
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_entity") {
        getEntityCalled = true;
        return Promise.resolve(taskBag);
      }
      if (cmd === "get_ui_state")
        return Promise.resolve({
          palette_open: false,
          palette_mode: "command",
          keymap_mode: "cua",
          scope_chain: [],
          open_boards: [],
          windows: {},
          recent_boards: [],
        });
      if (cmd === "list_schemas") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    await act(async () => {
      render(
        <RustEngineContainer>
          <EntityStoreProbe />
        </RustEngineContainer>,
      );
    });

    // Emit entity-created with EMPTY fields — fallback path
    await act(async () => {
      emitTauriEvent("entity-created", {
        kind: "entity-created",
        entity_type: "task",
        id: "t1",
        fields: {},
      });
    });

    await waitFor(() => {
      expect(screen.getByTestId("entity-store-ok").textContent).toBe(
        "entity-store-ok:1",
      );
    });

    // get_entity SHOULD have been called as fallback
    expect(getEntityCalled).toBe(true);
  });

  it("entity-removed event removes entities from the store", async () => {
    // Seed with an entity via refreshEntities
    const taskBag = {
      entity_type: "task",
      id: "t1",
      title: "Task",
    };
    mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === "get_entity") return Promise.resolve(taskBag);
      if (cmd === "get_ui_state")
        return Promise.resolve({
          palette_open: false,
          palette_mode: "command",
          keymap_mode: "cua",
          scope_chain: [],
          open_boards: [],
          windows: {},
          recent_boards: [],
        });
      if (cmd === "list_schemas") return Promise.resolve([]);
      if (cmd === "get_board_data")
        return Promise.resolve({
          board: { entity_type: "board", id: "b1", name: "Board" },
          columns: [],

          tags: [],
          summary: {
            total_tasks: 1,
            total_actors: 0,
            ready_tasks: 0,
            blocked_tasks: 0,
            done_tasks: 0,
            percent_complete: 0,
          },
        });
      if (cmd === "list_entities") {
        const a = args as Record<string, unknown>;
        if (a?.entityType === "task")
          return Promise.resolve({ entities: [taskBag] });
        return Promise.resolve({ entities: [] });
      }
      if (cmd === "list_open_boards")
        return Promise.resolve([
          { path: "/board", name: "Board", is_active: true },
        ]);
      return Promise.resolve(null);
    });

    let refreshFn: ((path: string) => void) | null = null;

    function CaptureRefresh() {
      const r = useRefreshEntities();
      refreshFn = r;
      return null;
    }

    await act(async () => {
      render(
        <RustEngineContainer>
          <EntityStoreProbe />
          <CaptureRefresh />
        </RustEngineContainer>,
      );
    });

    // Refresh to seed the store
    await act(async () => {
      await refreshFn!("/board");
    });

    await waitFor(() => {
      expect(screen.getByTestId("entity-store-ok").textContent).toBe(
        "entity-store-ok:1",
      );
    });

    // Now remove the entity
    await act(async () => {
      emitTauriEvent("entity-removed", {
        kind: "entity-removed",
        entity_type: "task",
        id: "t1",
      });
    });

    await waitFor(() => {
      expect(screen.getByTestId("entity-store-ok").textContent).toBe(
        "entity-store-ok:0",
      );
    });
  });

  it("entity-field-changed event updates existing entities", async () => {
    const taskBagV1 = {
      entity_type: "task",
      id: "t1",
      title: "Original",
    };
    const taskBagV2 = {
      entity_type: "task",
      id: "t1",
      title: "Updated",
    };

    let fetchCount = 0;
    mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === "get_entity") {
        fetchCount++;
        // First fetch returns v1, subsequent returns v2
        return Promise.resolve(fetchCount <= 1 ? taskBagV1 : taskBagV2);
      }
      if (cmd === "get_ui_state")
        return Promise.resolve({
          palette_open: false,
          palette_mode: "command",
          keymap_mode: "cua",
          scope_chain: [],
          open_boards: [],
          windows: {},
          recent_boards: [],
        });
      if (cmd === "list_schemas") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    /** Probe that displays the task title using reactive context. */
    function TitleProbe() {
      const entitiesByType = useEntitiesByType();
      const tasks = entitiesByType.task ?? [];
      const title =
        tasks.length > 0 ? String(tasks[0].fields.title ?? "") : "none";
      return <span data-testid="task-title">{title}</span>;
    }

    await act(async () => {
      render(
        <RustEngineContainer>
          <TitleProbe />
        </RustEngineContainer>,
      );
    });

    // Seed with entity-created
    await act(async () => {
      emitTauriEvent("entity-created", {
        kind: "entity-created",
        entity_type: "task",
        id: "t1",
        fields: {},
      });
    });

    await waitFor(() => {
      expect(screen.getByTestId("task-title").textContent).toBe("Original");
    });

    // Now trigger field-changed
    await act(async () => {
      emitTauriEvent("entity-field-changed", {
        kind: "entity-field-changed",
        entity_type: "task",
        id: "t1",
        changes: [{ field: "title", value: "Updated" }],
      });
    });

    await waitFor(() => {
      expect(screen.getByTestId("task-title").textContent).toBe("Updated");
    });
  });

  it("entity-field-changed with empty changes is a no-op", async () => {
    // Architecture rule: empty changes means nothing actually changed.
    // The handler should skip the event — no patching, no re-fetch.
    const taskBag = { entity_type: "task", id: "t1", title: "Original" };

    let getEntityCallCount = 0;
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_entity") {
        getEntityCallCount++;
        return Promise.resolve(taskBag);
      }
      if (cmd === "get_ui_state")
        return Promise.resolve({
          palette_open: false,
          palette_mode: "command",
          keymap_mode: "cua",
          scope_chain: [],
          open_boards: [],
          windows: {},
          recent_boards: [],
        });
      if (cmd === "list_schemas") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    /** Probe that displays the task title. */
    function TitleProbe() {
      const entitiesByType = useEntitiesByType();
      const tasks = entitiesByType.task ?? [];
      const title =
        tasks.length > 0 ? String(tasks[0].fields.title ?? "") : "none";
      return <span data-testid="task-title">{title}</span>;
    }

    await act(async () => {
      render(
        <RustEngineContainer>
          <TitleProbe />
        </RustEngineContainer>,
      );
    });

    // Seed with entity-created
    await act(async () => {
      emitTauriEvent("entity-created", {
        kind: "entity-created",
        entity_type: "task",
        id: "t1",
        fields: {},
      });
    });

    await waitFor(() => {
      expect(screen.getByTestId("task-title").textContent).toBe("Original");
    });

    const callsBefore = getEntityCallCount;

    // Emit entity-field-changed with EMPTY changes — should be skipped
    await act(async () => {
      emitTauriEvent("entity-field-changed", {
        kind: "entity-field-changed",
        entity_type: "task",
        id: "t1",
        changes: [],
      });
    });

    // Title should remain unchanged — no re-fetch triggered
    expect(screen.getByTestId("task-title").textContent).toBe("Original");
    // No additional get_entity calls
    expect(getEntityCallCount).toBe(callsBefore);
  });

  it("entity events with mismatched board_path are ignored", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_entity")
        return Promise.resolve({
          entity_type: "task",
          id: "t1",
          title: "Task",
        });
      if (cmd === "get_ui_state")
        return Promise.resolve({
          palette_open: false,
          palette_mode: "command",
          keymap_mode: "cua",
          scope_chain: [],
          open_boards: [],
          windows: {},
          recent_boards: [],
        });
      if (cmd === "list_schemas") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    let refreshFn: ((path: string) => void) | null = null;
    function CaptureRefresh() {
      const r = useRefreshEntities();
      refreshFn = r;
      return null;
    }

    await act(async () => {
      render(
        <RustEngineContainer>
          <EntityStoreProbe />
          <CaptureRefresh />
        </RustEngineContainer>,
      );
    });

    // Set active board path by calling refreshEntities
    await act(async () => {
      await refreshFn!("/active/board");
    });

    const countBefore = screen.getByTestId("entity-store-ok").textContent;

    // Emit event for a different board — should be ignored
    await act(async () => {
      emitTauriEvent("entity-created", {
        kind: "entity-created",
        entity_type: "task",
        id: "t99",
        fields: {},
        board_path: "/other/board",
      });
    });

    expect(screen.getByTestId("entity-store-ok").textContent).toBe(countBefore);
  });

  it("registers entity event listeners on mount", async () => {
    await act(async () => {
      render(
        <RustEngineContainer>
          <div>child</div>
        </RustEngineContainer>,
      );
    });

    // Check that listen was called for entity events
    const listenCalls = mockListen.mock.calls.map(
      (c: unknown[]) => c[0],
    ) as string[];
    expect(listenCalls).toContain("entity-created");
    expect(listenCalls).toContain("entity-removed");
    expect(listenCalls).toContain("entity-field-changed");
  });

  it("refreshEntities updates the store with fetched entities", async () => {
    const taskBag = {
      entity_type: "task",
      id: "t1",
      title: "Fetched Task",
    };
    mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === "get_ui_state")
        return Promise.resolve({
          palette_open: false,
          palette_mode: "command",
          keymap_mode: "cua",
          scope_chain: [],
          open_boards: [],
          windows: {},
          recent_boards: [],
        });
      if (cmd === "list_schemas") return Promise.resolve([]);
      if (cmd === "get_board_data")
        return Promise.resolve({
          board: { entity_type: "board", id: "b1", name: "Board" },
          columns: [],

          tags: [],
          summary: {
            total_tasks: 1,
            total_actors: 0,
            ready_tasks: 0,
            blocked_tasks: 0,
            done_tasks: 0,
            percent_complete: 0,
          },
        });
      if (cmd === "list_entities") {
        const a = args as Record<string, unknown>;
        if (a?.entityType === "task")
          return Promise.resolve({ entities: [taskBag] });
        return Promise.resolve({ entities: [] });
      }
      if (cmd === "list_open_boards")
        return Promise.resolve([
          { path: "/board", name: "Board", is_active: true },
        ]);
      return Promise.resolve(null);
    });

    let refreshFn: ((path: string) => void) | null = null;
    function CaptureRefresh() {
      const r = useRefreshEntities();
      refreshFn = r;
      return null;
    }

    await act(async () => {
      render(
        <RustEngineContainer>
          <EntityStoreProbe />
          <CaptureRefresh />
        </RustEngineContainer>,
      );
    });

    expect(screen.getByTestId("entity-store-ok").textContent).toBe(
      "entity-store-ok:0",
    );

    await act(async () => {
      await refreshFn!("/board");
    });

    await waitFor(() => {
      expect(screen.getByTestId("entity-store-ok").textContent).toBe(
        "entity-store-ok:1",
      );
    });
  });
});
