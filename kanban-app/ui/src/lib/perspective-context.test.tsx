import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import type { ReactNode } from "react";

// Mock Tauri APIs before importing any modules that use them.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn(
  (..._args: any[]): Promise<any> => Promise.resolve(null),
);
vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...args),
}));

// eslint-disable-next-line @typescript-eslint/no-explicit-any
let listenCallbacks: Record<string, (event: { payload: any }) => void> = {};
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(
    (
      event: string,
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      cb: (e: { payload: any }) => void,
    ) => {
      listenCallbacks[event] = cb;
      return Promise.resolve(() => {});
    },
  ),
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

// Mock UIState so we can control active_perspective_id.
let mockUIState = {
  keymap_mode: "cua",
  scope_chain: [],
  open_boards: [],
  has_clipboard: false,
  clipboard_entity_type: null,
  windows: {} as Record<string, { active_perspective_id: string }>,
  recent_boards: [],
};

vi.mock("./ui-state-context", () => ({
  useUIState: () => mockUIState,
}));

vi.mock("./views-context", () => ({
  useViews: () => ({
    views: [{ id: "board-1", name: "Board", kind: "board" }],
    activeView: { id: "board-1", name: "Board", kind: "board" },
    setActiveViewId: vi.fn(),
    refresh: vi.fn(() => Promise.resolve()),
  }),
}));

import { PerspectiveProvider, usePerspectives } from "./perspective-context";
import {
  CommandScopeProvider,
  ActiveBoardPathProvider,
} from "@/lib/command-scope";
import type { PerspectiveDef } from "@/types/kanban";

/** Build a minimal PerspectiveDef for test use. */
function makePerspective(id: string, name: string): PerspectiveDef {
  return { id, name, view: "board" };
}

/** Wrapper that renders PerspectiveProvider inside a CommandScopeProvider
 *  with a window moniker, matching the real App.tsx tree. */
function wrapper({ children }: { children: ReactNode }) {
  return (
    <CommandScopeProvider commands={[]} moniker="window:main">
      <ActiveBoardPathProvider value="/tmp/test/.kanban">
        <PerspectiveProvider>{children}</PerspectiveProvider>
      </ActiveBoardPathProvider>
    </CommandScopeProvider>
  );
}

describe("PerspectiveProvider", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listenCallbacks = {};
    mockUIState = {
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      has_clipboard: false,
      clipboard_entity_type: null,
      windows: {},
      recent_boards: [],
    };
    // Default: perspective.list returns empty (wrapped in dispatch envelope)
    mockInvoke.mockResolvedValue({
      result: { perspectives: [], count: 0 },
      undoable: false,
    });
  });

  it("provides empty perspectives list as default", async () => {
    const { result } = renderHook(() => usePerspectives(), { wrapper });
    await act(async () => {});
    expect(result.current.perspectives).toEqual([]);
    expect(result.current.activePerspective).toBeNull();
  });

  it("fetches perspectives on mount via perspective.list command", async () => {
    const ps = [makePerspective("p1", "Sprint View")];
    mockInvoke.mockResolvedValue({
      result: { perspectives: ps, count: 1 },
      undoable: false,
    });

    const { result } = renderHook(() => usePerspectives(), { wrapper });
    await act(async () => {});

    expect(mockInvoke).toHaveBeenCalledWith("dispatch_command", {
      cmd: "perspective.list",
      scopeChain: ["window:main"],
      boardPath: "/tmp/test/.kanban",
    });
    expect(result.current.perspectives).toHaveLength(1);
    expect(result.current.perspectives[0].name).toBe("Sprint View");
  });

  it("activePerspective falls back to first perspective when no id set", async () => {
    const ps = [
      makePerspective("p1", "First"),
      makePerspective("p2", "Second"),
    ];
    mockInvoke.mockResolvedValue({
      result: { perspectives: ps, count: 2 },
      undoable: false,
    });

    const { result } = renderHook(() => usePerspectives(), { wrapper });
    await act(async () => {});

    // No active_perspective_id in UIState → falls back to first
    expect(result.current.activePerspective?.id).toBe("p1");
  });

  it("activePerspective uses active_perspective_id from UIState", async () => {
    mockUIState = {
      ...mockUIState,
      windows: { main: { active_perspective_id: "p2" } },
    };

    const ps = [
      makePerspective("p1", "First"),
      makePerspective("p2", "Second"),
    ];
    mockInvoke.mockResolvedValue({
      result: { perspectives: ps, count: 2 },
      undoable: false,
    });

    const { result } = renderHook(() => usePerspectives(), { wrapper });
    await act(async () => {});

    expect(result.current.activePerspective?.id).toBe("p2");
    expect(result.current.activePerspective?.name).toBe("Second");
  });

  it("setActivePerspectiveId dispatches ui.perspective.set to backend", async () => {
    mockInvoke.mockResolvedValue({
      result: { perspectives: [], count: 0 },
      undoable: false,
    });

    const { result } = renderHook(() => usePerspectives(), { wrapper });
    await act(async () => {});

    // Clear the mount-time invoke call so we can assert cleanly
    mockInvoke.mockClear();
    mockInvoke.mockResolvedValue(null);

    await act(async () => {
      result.current.setActivePerspectiveId("p1");
      // Let the fire-and-forget promise settle
      await new Promise((r) => setTimeout(r, 0));
    });

    expect(mockInvoke).toHaveBeenCalledWith("dispatch_command", {
      cmd: "ui.perspective.set",
      args: { perspective_id: "p1" },
      scopeChain: ["window:main"],
      boardPath: "/tmp/test/.kanban",
    });
  });

  it("refresh re-fetches the perspectives list", async () => {
    mockInvoke.mockResolvedValue({
      result: { perspectives: [], count: 0 },
      undoable: false,
    });

    const { result } = renderHook(() => usePerspectives(), { wrapper });
    await act(async () => {});
    expect(result.current.perspectives).toHaveLength(0);

    // Now return a perspective on next fetch
    mockInvoke.mockResolvedValue({
      result: { perspectives: [makePerspective("p1", "New")], count: 1 },
      undoable: false,
    });

    await act(async () => {
      await result.current.refresh();
    });

    expect(result.current.perspectives).toHaveLength(1);
  });

  it("refetches perspective.list on entity-field-changed for a perspective", async () => {
    // The backend bridge emits `entity-field-changed` without usable field
    // values (every value is `Null` — the wire format is a signal, not a
    // patch). The listener responds by calling `perspective.list`, which
    // returns the freshest state from the canonical YAML.
    const initial = [
      makePerspective("p1", "First"),
      makePerspective("p2", "Second"),
    ];
    mockInvoke.mockResolvedValue({
      result: { perspectives: initial, count: 2 },
      undoable: false,
    });

    const { result } = renderHook(() => usePerspectives(), { wrapper });
    await act(async () => {});
    expect(result.current.perspectives).toHaveLength(2);

    // Next `perspective.list` call returns p1 with an updated name —
    // simulating a field edit that was just written to disk.
    const updated = [
      { ...makePerspective("p1", "Updated"), filter: "#bug" } as PerspectiveDef,
      makePerspective("p2", "Second"),
    ];
    mockInvoke.mockResolvedValue({
      result: { perspectives: updated, count: 2 },
      undoable: false,
    });

    const initialInvokeCount = mockInvoke.mock.calls.length;

    await act(async () => {
      listenCallbacks["entity-field-changed"]?.({
        payload: {
          entity_type: "perspective",
          id: "p1",
        },
      });
      await new Promise((r) => setTimeout(r, 0));
    });

    // The listener triggered a refetch, so `perspective.list` was invoked
    // once more than before the event.
    const listCallsAfter = mockInvoke.mock.calls.filter(
      (call) =>
        call[0] === "dispatch_command" &&
        (call[1] as { cmd: string })?.cmd === "perspective.list",
    ).length;
    const listCallsBefore = mockInvoke.mock.calls
      .slice(0, initialInvokeCount)
      .filter(
        (call) =>
          call[0] === "dispatch_command" &&
          (call[1] as { cmd: string })?.cmd === "perspective.list",
      ).length;
    expect(listCallsAfter).toBe(listCallsBefore + 1);

    // And the fresh state is reflected in the hook's perspectives.
    expect(result.current.perspectives.find((p) => p.id === "p1")?.name).toBe(
      "Updated",
    );
    expect(
      (result.current.perspectives.find((p) => p.id === "p1") as {
        filter?: unknown;
      } | undefined)?.filter,
    ).toBe("#bug");
  });

  it("refreshes on entity-created event for perspective type", async () => {
    mockInvoke.mockResolvedValue({
      result: { perspectives: [], count: 0 },
      undoable: false,
    });

    const { result } = renderHook(() => usePerspectives(), { wrapper });
    await act(async () => {});

    mockInvoke.mockResolvedValue({
      result: { perspectives: [makePerspective("p2", "Brand New")], count: 1 },
      undoable: false,
    });

    await act(async () => {
      listenCallbacks["entity-created"]?.({
        payload: { entity_type: "perspective", id: "p2" },
      });
      await new Promise((r) => setTimeout(r, 0));
    });

    expect(result.current.perspectives).toHaveLength(1);
    expect(result.current.perspectives[0].name).toBe("Brand New");
  });

  it("refreshes on entity-removed event for perspective type", async () => {
    const ps = [
      makePerspective("p1", "First"),
      makePerspective("p2", "Second"),
    ];
    mockInvoke.mockResolvedValue({
      result: { perspectives: ps, count: 2 },
      undoable: false,
    });

    const { result } = renderHook(() => usePerspectives(), { wrapper });
    await act(async () => {});
    expect(result.current.perspectives).toHaveLength(2);

    // After removal only one perspective remains
    mockInvoke.mockResolvedValue({
      result: { perspectives: [makePerspective("p1", "First")], count: 1 },
      undoable: false,
    });

    await act(async () => {
      listenCallbacks["entity-removed"]?.({
        payload: { entity_type: "perspective", id: "p2" },
      });
      await new Promise((r) => setTimeout(r, 0));
    });

    expect(result.current.perspectives).toHaveLength(1);
  });

  it("ignores entity events for non-perspective types", async () => {
    mockInvoke.mockResolvedValue({
      result: { perspectives: [], count: 0 },
      undoable: false,
    });

    const { result: _result } = renderHook(() => usePerspectives(), {
      wrapper,
    });
    await act(async () => {});

    const callCountBefore = mockInvoke.mock.calls.length;

    await act(async () => {
      listenCallbacks["entity-field-changed"]?.({
        payload: { entity_type: "task", id: "t1" },
      });
      await new Promise((r) => setTimeout(r, 0));
    });

    // No additional invoke calls for non-perspective events
    expect(mockInvoke.mock.calls.length).toBe(callCountBefore);
  });

  it("refreshes on board-changed event", async () => {
    mockInvoke.mockResolvedValue({
      result: { perspectives: [], count: 0 },
      undoable: false,
    });

    const { result } = renderHook(() => usePerspectives(), { wrapper });
    await act(async () => {});

    mockInvoke.mockResolvedValue({
      result: {
        perspectives: [makePerspective("p1", "After Board Change")],
        count: 1,
      },
      undoable: false,
    });

    await act(async () => {
      listenCallbacks["board-changed"]?.({ payload: undefined });
      await new Promise((r) => setTimeout(r, 0));
    });

    expect(result.current.perspectives).toHaveLength(1);
    expect(result.current.perspectives[0].name).toBe("After Board Change");
  });

  it("usePerspectives throws outside provider", () => {
    expect(() => renderHook(() => usePerspectives())).toThrow(
      "usePerspectives must be used within PerspectiveProvider",
    );
  });

  // -----------------------------------------------------------------------
  // useAutoSelectActivePerspective: enforce "always a selected perspective"
  // -----------------------------------------------------------------------

  /** Collect every `ui.perspective.set` dispatch recorded by the mock. */
  function perspectiveSetCalls() {
    return mockInvoke.mock.calls.filter(
      (call) =>
        call[0] === "dispatch_command" &&
        (call[1] as { cmd?: string })?.cmd === "ui.perspective.set",
    );
  }

  it("auto-selects the first matching perspective when UIState active id is empty", async () => {
    // UIState active_perspective_id empty; one board perspective exists.
    const ps = [makePerspective("p1", "Sprint")];
    mockInvoke.mockResolvedValue({
      result: { perspectives: ps, count: 1 },
      undoable: false,
    });

    renderHook(() => usePerspectives(), { wrapper });

    // Wait for effects + the dispatch to settle.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });

    const calls = perspectiveSetCalls();
    expect(calls.length).toBeGreaterThanOrEqual(1);
    expect(calls[0][1]).toMatchObject({
      cmd: "ui.perspective.set",
      args: { perspective_id: "p1" },
    });
  });

  it("auto-selects the first matching perspective when active id names a deleted perspective", async () => {
    // UIState points at "gone"; the list no longer contains it.
    mockUIState = {
      ...mockUIState,
      windows: { main: { active_perspective_id: "gone" } },
    };

    const ps = [makePerspective("p1", "Survivor")];
    mockInvoke.mockResolvedValue({
      result: { perspectives: ps, count: 1 },
      undoable: false,
    });

    renderHook(() => usePerspectives(), { wrapper });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });

    const calls = perspectiveSetCalls();
    expect(calls.length).toBeGreaterThanOrEqual(1);
    expect(calls[0][1]).toMatchObject({
      cmd: "ui.perspective.set",
      args: { perspective_id: "p1" },
    });
  });

  it("auto-selects the first matching perspective when active id is for a different view kind", async () => {
    // UIState active points at a grid perspective; the current view is board.
    mockUIState = {
      ...mockUIState,
      windows: { main: { active_perspective_id: "g1" } },
    };

    const ps = [
      { ...makePerspective("g1", "Grid Only"), view: "grid" },
      makePerspective("b1", "Board One"),
    ];
    mockInvoke.mockResolvedValue({
      result: { perspectives: ps, count: 2 },
      undoable: false,
    });

    renderHook(() => usePerspectives(), { wrapper });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });

    const calls = perspectiveSetCalls();
    expect(calls.length).toBeGreaterThanOrEqual(1);
    expect(calls[0][1]).toMatchObject({
      cmd: "ui.perspective.set",
      args: { perspective_id: "b1" },
    });
  });

  it("does NOT auto-select when the active id is already a valid matching perspective", async () => {
    mockUIState = {
      ...mockUIState,
      windows: { main: { active_perspective_id: "p2" } },
    };

    const ps = [
      makePerspective("p1", "First"),
      makePerspective("p2", "Second"),
    ];
    mockInvoke.mockResolvedValue({
      result: { perspectives: ps, count: 2 },
      undoable: false,
    });

    renderHook(() => usePerspectives(), { wrapper });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });

    // Auto-selection must not fire when the stored id is already valid.
    expect(perspectiveSetCalls().length).toBe(0);
  });

  it("does NOT auto-select when no perspectives exist for the view kind (let auto-create handle it)", async () => {
    // No perspectives at all — useAutoCreateDefaultPerspective should fire
    // instead; useAutoSelectActivePerspective must bail out.
    mockInvoke.mockResolvedValue({
      result: { perspectives: [], count: 0 },
      undoable: false,
    });

    renderHook(() => usePerspectives(), { wrapper });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });

    // No ui.perspective.set — only perspective.save would be dispatched by
    // the sibling hook (not asserted here; covered elsewhere).
    expect(perspectiveSetCalls().length).toBe(0);
  });

  // -----------------------------------------------------------------------
  // Post-undo refresh behavior
  // -----------------------------------------------------------------------
  //
  // The Tauri bridge (kanban-app/src/watcher.rs::process_perspective_event)
  // translates a backend `PerspectiveEvent::PerspectiveChanged` into an
  // `entity-field-changed` Tauri event. After the kanban-local `app.undo`
  // wrapper calls `PerspectiveContext::reload_from_disk`, the bridge emits
  // that event WITHOUT a `fields` key (the backend's wire shape uses
  // `changes` and the empty `changed_fields` list signals "full refresh").
  //
  // The frontend listener's contract is: "no `fields` → full refetch". That
  // refetch is the last step in the undo chain that makes the UI actually
  // show the reverted group/filter/sort. These tests pin that behavior so
  // any future optimization of the field-delta fast path doesn't
  // accidentally drop the undo refresh.
  it("refetches perspective.list on entity-field-changed without fields (post-undo shape)", async () => {
    // Start with a grouped perspective — simulates the state immediately
    // after `perspective.group` succeeded but before the user invokes Undo.
    const before = [
      { ...makePerspective("p1", "Sprint"), group: "status" },
    ] as PerspectiveDef[];
    mockInvoke.mockResolvedValue({
      result: { perspectives: before, count: 1 },
      undoable: false,
    });

    const { result } = renderHook(() => usePerspectives(), { wrapper });
    await act(async () => {});
    expect(
      (result.current.perspectives[0] as { group?: unknown }).group,
    ).toBe("status");

    // After undo, the backend has rewritten the YAML back to group=None.
    // Prime the next perspective.list response to reflect that state.
    const after = [makePerspective("p1", "Sprint")] as PerspectiveDef[];
    mockInvoke.mockResolvedValue({
      result: { perspectives: after, count: 1 },
      undoable: false,
    });

    // Fire the entity-field-changed event with no `fields` key — the exact
    // shape the bridge emits after reload_from_disk with changed_fields=[].
    // The listener must detect the missing fields and trigger a refresh().
    await act(async () => {
      listenCallbacks["entity-field-changed"]?.({
        payload: {
          entity_type: "perspective",
          id: "p1",
          // no fields / no changes — simulating the post-undo wire shape
        },
      });
      await new Promise((r) => setTimeout(r, 0));
    });

    // The refetch must have happened — `group` is now undefined.
    expect(
      (result.current.perspectives[0] as { group?: unknown }).group,
    ).toBeUndefined();
  });

  it("refetches on entity-removed (post-undo-of-create shape)", async () => {
    // Start with one perspective — simulates "just created".
    const before = [makePerspective("p1", "Ephemeral")];
    mockInvoke.mockResolvedValue({
      result: { perspectives: before, count: 1 },
      undoable: false,
    });

    const { result } = renderHook(() => usePerspectives(), { wrapper });
    await act(async () => {});
    expect(result.current.perspectives).toHaveLength(1);

    // Undo the create — the backend deletes the file and `reload_from_disk`
    // emits `PerspectiveDeleted`, which the bridge maps to `entity-removed`.
    mockInvoke.mockResolvedValue({
      result: { perspectives: [], count: 0 },
      undoable: false,
    });

    await act(async () => {
      listenCallbacks["entity-removed"]?.({
        payload: { entity_type: "perspective", id: "p1" },
      });
      await new Promise((r) => setTimeout(r, 0));
    });

    expect(result.current.perspectives).toHaveLength(0);
  });
});
