import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import type { ReactNode } from "react";

/* ---- Tauri mocks ---- */

const mockInvoke = vi.fn((..._args: unknown[]) => Promise.resolve({}));
let listenHandlers: Record<string, (event: { payload: unknown }) => void> = {};

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((event: string, handler: (e: { payload: unknown }) => void) => {
    listenHandlers[event] = handler;
    return Promise.resolve(() => {
      delete listenHandlers[event];
    });
  }),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

vi.mock("@/lib/command-scope", async () => {
  const actual = await vi.importActual("@/lib/command-scope");
  return {
    ...actual,
  };
});

import {
  DragSessionProvider,
  useDragSession,
  type DragSession,
} from "./drag-session-context";

/* ---- Helpers ---- */

function wrapper({ children }: { children: ReactNode }) {
  return <DragSessionProvider>{children}</DragSessionProvider>;
}

function makeSession(overrides: Partial<DragSession> = {}): DragSession {
  return {
    session_id: "sess-1",
    source_board_path: "/board/a/.kanban",
    source_window_label: "main",
    task_id: "task-1",
    task_fields: { title: "Test task" },
    copy_mode: false,
    from: {
      kind: "focus_chain",
      entity_type: "task",
      entity_id: "task-1",
      fields: { title: "Test task" },
      source_board_path: "/board/a/.kanban",
      source_window_label: "main",
    },
    ...overrides,
  };
}

/**
 * File-source session payload. External file drags leave the legacy
 * flat task fields empty; the `from.kind === "file"` envelope is the
 * authoritative source of truth for file drops.
 */
function makeFileSession(overrides: Partial<DragSession> = {}): DragSession {
  return {
    session_id: "file-sess-1",
    source_board_path: "",
    source_window_label: "main",
    task_id: "",
    task_fields: {},
    copy_mode: false,
    from: {
      kind: "file",
      path: "/tmp/dropped.png",
    },
    ...overrides,
  };
}

/** Simulate a Tauri event by calling the registered listener. */
function emitEvent(name: string, payload: unknown) {
  const handler = listenHandlers[name];
  if (handler) handler({ payload });
}

describe("DragSessionProvider", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listenHandlers = {};
  });

  it("starts with no session and isSource false", () => {
    const { result } = renderHook(() => useDragSession(), { wrapper });
    expect(result.current.session).toBeNull();
    expect(result.current.isSource).toBe(false);
  });

  it("sets session on drag-session-active event", () => {
    const { result } = renderHook(() => useDragSession(), { wrapper });
    const session = makeSession();

    act(() => emitEvent("drag-session-active", session));

    expect(result.current.session).toEqual(session);
  });

  it("sets isSource true when source_window_label matches this window", () => {
    const { result } = renderHook(() => useDragSession(), { wrapper });
    const session = makeSession({ source_window_label: "main" });

    act(() => emitEvent("drag-session-active", session));

    expect(result.current.isSource).toBe(true);
  });

  it("sets isSource false when source_window_label is a different window", () => {
    const { result } = renderHook(() => useDragSession(), { wrapper });
    const session = makeSession({ source_window_label: "board-1" });

    act(() => emitEvent("drag-session-active", session));

    expect(result.current.isSource).toBe(false);
  });

  it("clears session on drag-session-cancelled event", () => {
    const { result } = renderHook(() => useDragSession(), { wrapper });

    act(() => emitEvent("drag-session-active", makeSession()));
    expect(result.current.session).not.toBeNull();

    act(() => emitEvent("drag-session-cancelled", { session_id: "sess-1" }));
    expect(result.current.session).toBeNull();
    expect(result.current.isSource).toBe(false);
  });

  it("clears session on drag-session-completed event", () => {
    const { result } = renderHook(() => useDragSession(), { wrapper });

    act(() => emitEvent("drag-session-active", makeSession()));
    expect(result.current.session).not.toBeNull();

    act(() =>
      emitEvent("drag-session-completed", {
        session_id: "sess-1",
        success: true,
      }),
    );
    expect(result.current.session).toBeNull();
    expect(result.current.isSource).toBe(false);
  });

  it("startSession invokes dispatch_command drag.start with correct params", async () => {
    mockInvoke.mockResolvedValue({
      result: { DragStart: { session_id: "new-sess" } },
      undoable: false,
    });
    const { result } = renderHook(() => useDragSession(), { wrapper });

    await act(async () => {
      await result.current.startSession("task-42", { title: "My task" }, false);
    });

    expect(mockInvoke).toHaveBeenCalledWith("dispatch_command", {
      cmd: "drag.start",
      args: {
        taskId: "task-42",
        taskFields: { title: "My task" },
        sourceWindowLabel: "main",
        copyMode: false,
      },
      scopeChain: [],
    });
  });

  it("cancelSession invokes dispatch_command with drag.cancel", async () => {
    mockInvoke.mockResolvedValue({ result: null, undoable: false });
    const { result } = renderHook(() => useDragSession(), { wrapper });

    await act(async () => {
      await result.current.cancelSession();
    });

    expect(mockInvoke).toHaveBeenCalledWith("dispatch_command", {
      cmd: "drag.cancel",
      scopeChain: [],
    });
  });

  it("completeSession invokes dispatch_command drag.complete with options", async () => {
    mockInvoke.mockResolvedValue({ result: {} });
    const { result } = renderHook(() => useDragSession(), { wrapper });

    await act(async () => {
      await result.current.completeSession("done", {
        dropIndex: 3,
        beforeId: "task-5",
        copyMode: true,
      });
    });

    expect(mockInvoke).toHaveBeenCalledWith("dispatch_command", {
      cmd: "drag.complete",
      args: {
        targetColumn: "done",
        dropIndex: 3,
        beforeId: "task-5",
        afterId: null,
        copyMode: true,
      },
      scopeChain: [],
    });
  });

  it("completeSession defaults optional params to null/false", async () => {
    mockInvoke.mockResolvedValue({ result: {} });
    const { result } = renderHook(() => useDragSession(), { wrapper });

    await act(async () => {
      await result.current.completeSession("todo");
    });

    expect(mockInvoke).toHaveBeenCalledWith("dispatch_command", {
      cmd: "drag.complete",
      args: {
        targetColumn: "todo",
        dropIndex: null,
        beforeId: null,
        afterId: null,
        copyMode: false,
      },
      scopeChain: [],
    });
  });

  // ---------------------------------------------------------------------
  // File-source drag (card 01KPNGPRBQACX5ZPZEX414Z68R): external OS file
  // dragged into the app is "paste by another name" — the session's
  // `from.kind === "file"` envelope is the authoritative shape.
  // ---------------------------------------------------------------------

  it("exposes from.kind: 'file' on a file-source drag-session-active event", () => {
    const { result } = renderHook(() => useDragSession(), { wrapper });
    const fileSession = makeFileSession({
      from: { kind: "file", path: "/tmp/example/screenshot.png" },
    });

    act(() => emitEvent("drag-session-active", fileSession));

    const session = result.current.session;
    expect(session).not.toBeNull();
    // Narrow on the discriminant before asserting variant fields.
    if (session?.from.kind !== "file") {
      throw new Error("expected from.kind === 'file'");
    }
    expect(session.from.path).toBe("/tmp/example/screenshot.png");
    // Legacy flat focus-chain fields are empty for file drags.
    expect(session.task_id).toBe("");
    expect(session.source_board_path).toBe("");
  });

  it("preserves from.kind: 'focus_chain' on a task drag-session-active event", () => {
    const { result } = renderHook(() => useDragSession(), { wrapper });
    const taskSession = makeSession();

    act(() => emitEvent("drag-session-active", taskSession));

    const session = result.current.session;
    expect(session).not.toBeNull();
    if (session?.from.kind !== "focus_chain") {
      throw new Error("expected from.kind === 'focus_chain'");
    }
    expect(session.from.entity_type).toBe("task");
    expect(session.from.entity_id).toBe("task-1");
    expect(session.from.source_board_path).toBe("/board/a/.kanban");
  });

  it("startFileSession invokes dispatch_command drag.start with sourceKind=file", async () => {
    mockInvoke.mockResolvedValue({
      result: { DragStart: { session_id: "file-sess" } },
      undoable: false,
    });
    const { result } = renderHook(() => useDragSession(), { wrapper });

    await act(async () => {
      await result.current.startFileSession("/tmp/dropped.png");
    });

    expect(mockInvoke).toHaveBeenCalledWith("dispatch_command", {
      cmd: "drag.start",
      args: {
        sourceKind: "file",
        filePath: "/tmp/dropped.png",
        sourceWindowLabel: "main",
        copyMode: false,
      },
      scopeChain: [],
    });
  });

  it("startFileSession forwards copyMode when provided", async () => {
    mockInvoke.mockResolvedValue({
      result: { DragStart: { session_id: "file-sess-2" } },
      undoable: false,
    });
    const { result } = renderHook(() => useDragSession(), { wrapper });

    await act(async () => {
      await result.current.startFileSession("/tmp/alt.png", true);
    });

    expect(mockInvoke).toHaveBeenCalledWith("dispatch_command", {
      cmd: "drag.start",
      args: {
        sourceKind: "file",
        filePath: "/tmp/alt.png",
        sourceWindowLabel: "main",
        copyMode: true,
      },
      scopeChain: [],
    });
  });

  it("completeFileSession dispatches drag.complete with target moniker and no column args", async () => {
    mockInvoke.mockResolvedValue({ result: {} });
    const { result } = renderHook(() => useDragSession(), { wrapper });

    await act(async () => {
      await result.current.completeFileSession("task:01ABC");
    });

    // File drags don't carry a targetColumn — the drop destination is
    // the entity moniker itself, read from `target` by DragCompleteCmd.
    expect(mockInvoke).toHaveBeenCalledWith("dispatch_command", {
      cmd: "drag.complete",
      target: "task:01ABC",
      scopeChain: [],
    });
  });

  it("isSource uses window label not board path for same-board multi-window", () => {
    const { result } = renderHook(() => useDragSession(), { wrapper });

    // Same board path, different window label → NOT source
    act(() =>
      emitEvent(
        "drag-session-active",
        makeSession({
          source_board_path: "/board/a/.kanban",
          source_window_label: "board-2",
        }),
      ),
    );

    expect(result.current.isSource).toBe(false);
    expect(result.current.session?.source_board_path).toBe("/board/a/.kanban");
  });
});
