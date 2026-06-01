import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useState, type ReactNode } from "react";

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

// Mock UIState so we can control active_perspective_id (and, optionally,
// filtered_task_ids — see the boot-recovery test). The
// `filtered_task_ids` slot is intentionally optional here to model the
// tri-state contract: `undefined` means "no perspective.switch has fired
// yet for this window".
let mockUIState = {
  keymap_mode: "cua",
  scope_chain: [],
  open_boards: [],
  has_clipboard: false,
  clipboard_entity_type: null,
  windows: {} as Record<
    string,
    { active_perspective_id: string; filtered_task_ids?: string[] }
  >,
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
  FocusedScopeContext,
  type CommandScope,
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

  it("setActivePerspectiveId dispatches perspective.switch to backend", async () => {
    // After 01KP3ERHEDP86C2JYYR7NM1593, `setActivePerspectiveId` issues
    // `perspective.switch` — the single backend command that atomically
    // sets `active_perspective_id` AND `filtered_task_ids`. The legacy
    // `perspective.set` (id-only) is gone. The args shape is unchanged:
    // `{ perspective_id }` flows on `args`.
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
      cmd: "perspective.switch",
      args: { perspective_id: "p1" },
      scopeChain: ["window:main"],
      boardPath: "/tmp/test/.kanban",
    });

    // Regression guard: the legacy command name must NOT be dispatched.
    const legacy = mockInvoke.mock.calls.find(
      (call) =>
        call[0] === "dispatch_command" &&
        (call[1] as { cmd?: string })?.cmd === "perspective.set",
    );
    expect(legacy).toBeUndefined();
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

    // `subscribeStoreChanged` does a lazy `import("@tauri-apps/api/event")`
    // followed by a `.then(listen)`. Under the full-suite scheduler this
    // chain can need multiple microtask + setTimeout hops to resolve, so
    // wait until the listener has actually been registered before firing
    // the event — otherwise the optional-chain on the callback silently
    // swallows the dispatch and the refetch never happens.
    await act(async () => {
      for (let i = 0; i < 50; i++) {
        if (listenCallbacks["notifications/store/changed"]) break;
        await new Promise((r) => setTimeout(r, 0));
      }
    });
    expect(listenCallbacks["notifications/store/changed"]).toBeDefined();

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
      listenCallbacks["notifications/store/changed"]?.({
        payload: {
          store: "perspective",
          item: "p1",
          op: "updated",
          txn: null,
          origin: "user",
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
      (
        result.current.perspectives.find((p) => p.id === "p1") as
          | {
              filter?: unknown;
            }
          | undefined
      )?.filter,
    ).toBe("#bug");
  });

  it("refreshes on a store/changed created notification for perspective store", async () => {
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
      listenCallbacks["notifications/store/changed"]?.({
        payload: {
          store: "perspective",
          item: "p2",
          op: "created",
          txn: null,
          origin: "user",
        },
      });
      await new Promise((r) => setTimeout(r, 0));
    });

    expect(result.current.perspectives).toHaveLength(1);
    expect(result.current.perspectives[0].name).toBe("Brand New");
  });

  it("refreshes on a store/changed removed notification for perspective store", async () => {
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
      listenCallbacks["notifications/store/changed"]?.({
        payload: {
          store: "perspective",
          item: "p2",
          op: "removed",
          txn: null,
          origin: "user",
        },
      });
      await new Promise((r) => setTimeout(r, 0));
    });

    expect(result.current.perspectives).toHaveLength(1);
  });

  it("ignores store/changed notifications for non-perspective stores", async () => {
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
      listenCallbacks["notifications/store/changed"]?.({
        payload: {
          store: "task",
          item: "t1",
          op: "updated",
          changes: [{ field: "title", value: "x" }],
          txn: null,
          origin: "user",
        },
      });
      await new Promise((r) => setTimeout(r, 0));
    });

    // No additional invoke calls for non-perspective stores
    expect(mockInvoke.mock.calls.length).toBe(callCountBefore);
  });

  it("refreshes on a structural board store/changed notification", async () => {
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
      listenCallbacks["notifications/store/changed"]?.({
        payload: {
          store: "board",
          item: "b1",
          op: "updated",
          changes: [{ field: "name", value: "x" }],
          txn: null,
          origin: "user",
        },
      });
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

  /** Collect every `perspective.switch` dispatch recorded by the mock. */
  function perspectiveSwitchCalls() {
    return mockInvoke.mock.calls.filter(
      (call) =>
        call[0] === "dispatch_command" &&
        (call[1] as { cmd?: string })?.cmd === "perspective.switch",
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

    const calls = perspectiveSwitchCalls();
    expect(calls.length).toBeGreaterThanOrEqual(1);
    expect(calls[0][1]).toMatchObject({
      cmd: "perspective.switch",
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

    const calls = perspectiveSwitchCalls();
    expect(calls.length).toBeGreaterThanOrEqual(1);
    expect(calls[0][1]).toMatchObject({
      cmd: "perspective.switch",
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

    const calls = perspectiveSwitchCalls();
    expect(calls.length).toBeGreaterThanOrEqual(1);
    expect(calls[0][1]).toMatchObject({
      cmd: "perspective.switch",
      args: { perspective_id: "b1" },
    });
  });

  it("does NOT auto-select when the active id is already a valid matching perspective AND filtered_task_ids is populated", async () => {
    // Steady-state: the window has both a valid `active_perspective_id` AND
    // a defined `filtered_task_ids` slot (i.e. a `perspective.switch` has
    // already fired this session). The auto-select hook must NOT redispatch.
    mockUIState = {
      ...mockUIState,
      windows: {
        main: {
          active_perspective_id: "p2",
          filtered_task_ids: ["t1", "t2"],
        },
      },
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

    // Auto-selection must not fire when the stored id is already valid AND
    // the filter has already been computed for it.
    expect(perspectiveSwitchCalls().length).toBe(0);
  });

  it("auto-recovers stale boot state: persisted active_perspective_id with undefined filtered_task_ids redispatches perspective.switch for the persisted id", async () => {
    // Boot-time regression scenario: `WindowState::filtered_task_ids` is
    // `#[serde(skip)]` on the backend, so a restart with a persisted
    // `active_perspective_id` lands here with `filtered_task_ids` not yet
    // populated (undefined). The tri-state contract in `view-container.tsx`
    // treats `undefined` as "no filter — show all", but the persisted id
    // means the user DOES expect a filter; without redispatch they would
    // see all tasks unfiltered.
    //
    // The auto-select hook treats this as stale state and redispatches
    // `perspective.switch` for the persisted id (not the first matching
    // perspective — we honor the user's prior selection). The backend then
    // recomputes and pushes both `active_perspective_id` and
    // `filtered_task_ids` back atomically.
    mockUIState = {
      ...mockUIState,
      // Note: no `filtered_task_ids` key → undefined on the snapshot.
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

    const calls = perspectiveSwitchCalls();
    expect(calls.length).toBeGreaterThanOrEqual(1);
    expect(calls[0][1]).toMatchObject({
      cmd: "perspective.switch",
      args: { perspective_id: "p2" }, // The persisted id, not "p1".
    });
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

    // No perspective.switch — only perspective.save would be dispatched by
    // the sibling hook (not asserted here; covered elsewhere).
    expect(perspectiveSwitchCalls().length).toBe(0);
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
  it("refetches perspective.list on a reload-item store/changed (post-undo shape)", async () => {
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
    expect((result.current.perspectives[0] as { group?: unknown }).group).toBe(
      "status",
    );

    // After undo, the backend has rewritten the YAML back to group=None.
    // Prime the next perspective.list response to reflect that state.
    const after = [makePerspective("p1", "Sprint")] as PerspectiveDef[];
    mockInvoke.mockResolvedValue({
      result: { perspectives: after, count: 1 },
      undoable: false,
    });

    // Fire the reload-item store/changed for the perspective store — the wire
    // shape omits `changes` (perspectives re-fetch from canonical YAML). The
    // subscriber must treat it as a refetch signal.
    await act(async () => {
      listenCallbacks["notifications/store/changed"]?.({
        payload: {
          store: "perspective",
          item: "p1",
          op: "updated",
          txn: null,
          origin: "undo",
          // no `changes` — reload-item semantics
        },
      });
      await new Promise((r) => setTimeout(r, 0));
    });

    // The refetch must have happened — `group` is now undefined.
    expect(
      (result.current.perspectives[0] as { group?: unknown }).group,
    ).toBeUndefined();
  });

  it("refetches on a store/changed removed (post-undo-of-create shape)", async () => {
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
      listenCallbacks["notifications/store/changed"]?.({
        payload: {
          store: "perspective",
          item: "p1",
          op: "removed",
          txn: null,
          origin: "undo",
        },
      });
      await new Promise((r) => setTimeout(r, 0));
    });

    expect(result.current.perspectives).toHaveLength(0);
  });

  // -----------------------------------------------------------------------
  // Focus-scope regression: perspective.list must NOT refetch on focus
  // -----------------------------------------------------------------------
  //
  // `useDispatchCommand` memoizes its returned callback with `effectiveScope =
  // focusedScope ?? treeScope` in its dep array. Every grid keystroke fires
  // `ui.setFocus`, which rotates `FocusedScopeContext` and therefore the
  // `dispatch` identity. If `PerspectiveProvider`'s hooks close over
  // `dispatch` in `useCallback`/`useEffect` deps, a new `dispatch` identity
  // triggers a full `perspective.list` refetch — and churns the companion
  // auto-create / auto-select effects as well.
  //
  // Perspectives are a per-view concern, not per-focused-element. They should
  // only refetch on mount (once per view kind) and in response to backend
  // events (`entity-created` / `entity-field-changed` / `entity-removed` for
  // perspectives, and `board-changed`).
  //
  // The fix is a `dispatchRef` pattern in `perspective-context.tsx`: hooks
  // read `dispatchRef.current` instead of closing over `dispatch`, and their
  // dep arrays no longer mention `dispatch`. These tests pin that behavior.

  /** Count how many times `dispatch_command` was invoked with a given cmd id. */
  function invokeCallsFor(cmd: string): number {
    return mockInvoke.mock.calls.filter(
      (call) =>
        call[0] === "dispatch_command" &&
        (call[1] as { cmd?: string })?.cmd === cmd,
    ).length;
  }

  /**
   * Module-scoped setter captured during render so individual tests can
   * toggle the focused scope value from outside React. Matches how
   * EntityFocusProvider flips `FocusedScopeContext` on every `ui.setFocus`.
   */
  let setFocusedScopeExternal: ((next: CommandScope | null) => void) | null =
    null;

  /**
   * Wrapper that exposes a toggleable `FocusedScopeContext` value. Calling
   * `setFocusedScopeExternal` from a test simulates an entity focus change.
   */
  function FocusToggleWrapper({ children }: { children: ReactNode }) {
    const [focused, setFocused] = useState<CommandScope | null>(null);
    setFocusedScopeExternal = setFocused;
    return (
      <CommandScopeProvider commands={[]} moniker="window:main">
        <ActiveBoardPathProvider value="/tmp/test/.kanban">
          <FocusedScopeContext.Provider value={focused}>
            <PerspectiveProvider>{children}</PerspectiveProvider>
          </FocusedScopeContext.Provider>
        </ActiveBoardPathProvider>
      </CommandScopeProvider>
    );
  }

  /**
   * Build a synthetic focused scope with a fresh moniker so each toggle
   * changes identity — matching how EntityFocusProvider produces a new
   * `focusedScope` object whenever focus moves.
   */
  function makeFocusedScope(moniker: string): CommandScope {
    return { commands: new Map(), parent: null, moniker };
  }

  it("does not refetch perspective.list when focused scope changes", async () => {
    mockInvoke.mockResolvedValue({
      result: { perspectives: [makePerspective("p1", "Sprint")], count: 1 },
      undoable: false,
    });

    renderHook(() => usePerspectives(), { wrapper: FocusToggleWrapper });

    // Let mount-time fetch + auto-select settle.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });

    const listCallsBefore = invokeCallsFor("perspective.list");
    expect(listCallsBefore).toBe(1);

    // Simulate a sequence of focus changes — exactly what arrow-key navigation
    // produces in production: each `ui.setFocus` rotates FocusedScopeContext.
    await act(async () => {
      setFocusedScopeExternal?.(makeFocusedScope("task:t1"));
      await new Promise((r) => setTimeout(r, 0));
    });
    await act(async () => {
      setFocusedScopeExternal?.(makeFocusedScope("task:t2"));
      await new Promise((r) => setTimeout(r, 0));
    });
    await act(async () => {
      setFocusedScopeExternal?.(makeFocusedScope("task:t3"));
      await new Promise((r) => setTimeout(r, 0));
    });

    // No extra `perspective.list` dispatches — refetch is reserved for
    // backend events, not focus changes.
    expect(invokeCallsFor("perspective.list")).toBe(listCallsBefore);
  });

  it("does not re-run auto-create effect when focused scope changes", async () => {
    // Empty perspective list on mount triggers auto-create exactly once.
    mockInvoke.mockResolvedValue({
      result: { perspectives: [], count: 0 },
      undoable: false,
    });

    renderHook(() => usePerspectives(), { wrapper: FocusToggleWrapper });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });

    const saveCallsBefore = invokeCallsFor("perspective.save");
    // Auto-create fires once on mount when no perspectives exist for the kind.
    expect(saveCallsBefore).toBe(1);

    await act(async () => {
      setFocusedScopeExternal?.(makeFocusedScope("task:t1"));
      await new Promise((r) => setTimeout(r, 0));
    });
    await act(async () => {
      setFocusedScopeExternal?.(makeFocusedScope("task:t2"));
      await new Promise((r) => setTimeout(r, 0));
    });

    // Focus changes must not re-enter the auto-create effect — the per-kind
    // ref guard already prevents a second save, but we also want to be sure
    // the effect body itself does not run.
    expect(invokeCallsFor("perspective.save")).toBe(saveCallsBefore);
  });

  it("does not re-run auto-select effect when focused scope changes", async () => {
    // Stale active id forces auto-select to run once on mount.
    mockUIState = {
      ...mockUIState,
      windows: { main: { active_perspective_id: "gone" } },
    };

    const ps = [makePerspective("p1", "Survivor")];
    mockInvoke.mockResolvedValue({
      result: { perspectives: ps, count: 1 },
      undoable: false,
    });

    renderHook(() => usePerspectives(), { wrapper: FocusToggleWrapper });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });

    const setCallsBefore = invokeCallsFor("perspective.switch");
    expect(setCallsBefore).toBeGreaterThanOrEqual(1);

    await act(async () => {
      setFocusedScopeExternal?.(makeFocusedScope("task:t1"));
      await new Promise((r) => setTimeout(r, 0));
    });
    await act(async () => {
      setFocusedScopeExternal?.(makeFocusedScope("task:t2"));
      await new Promise((r) => setTimeout(r, 0));
    });

    // Focus changes must not redispatch `perspective.switch`. The stored id
    // is unchanged, the view kind is unchanged, and the perspectives list
    // is unchanged — there is nothing to reconcile.
    expect(invokeCallsFor("perspective.switch")).toBe(setCallsBefore);
  });
});
