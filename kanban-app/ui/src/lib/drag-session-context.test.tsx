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

vi.mock("@/lib/command-scope", () => ({
  useActiveBoardPath: () => "/board/a/.kanban",
}));

import { DragSessionProvider, useDragSession, type DragSession } from "./drag-session-context";

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
    mockInvoke.mockResolvedValue({ result: { DragStart: { session_id: "new-sess" } }, undoable: false });
    const { result } = renderHook(() => useDragSession(), { wrapper });

    await act(async () => {
      await result.current.startSession("task-42", { title: "My task" }, false);
    });

    expect(mockInvoke).toHaveBeenCalledWith("dispatch_command", {
      cmd: "drag.start",
      args: {
        taskId: "task-42",
        taskFields: { title: "My task" },
        boardPath: "/board/a/.kanban",
        sourceWindowLabel: "main",
        copyMode: false,
      },
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
        targetBoardPath: "/board/a/.kanban",
        targetColumn: "done",
        dropIndex: 3,
        beforeId: "task-5",
        afterId: null,
        copyMode: true,
      },
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
        targetBoardPath: "/board/a/.kanban",
        targetColumn: "todo",
        dropIndex: null,
        beforeId: null,
        afterId: null,
        copyMode: false,
      },
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
