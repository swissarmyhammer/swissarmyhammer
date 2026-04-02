import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import type { ReactNode } from "react";

// ---------------------------------------------------------------------------
// Mocks — must be before component imports
// ---------------------------------------------------------------------------

/** Captured callback from onDragDropEvent so tests can simulate events. */
let dragDropCallback:
  | ((event: { payload: { type: string; paths?: string[] } }) => void)
  | null = null;
const mockUnlisten = vi.fn();

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: vi.fn(
      (
        cb: (event: { payload: { type: string; paths?: string[] } }) => void,
      ) => {
        dragDropCallback = cb;
        return Promise.resolve(mockUnlisten);
      },
    ),
  }),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve(null)),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
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
// Imports — after mocks
// ---------------------------------------------------------------------------

import { FileDropProvider, useFileDrop } from "./file-drop-context";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function wrapper({ children }: { children: ReactNode }) {
  return <FileDropProvider>{children}</FileDropProvider>;
}

/** Simulate a Tauri drag-drop event. */
function emitDragDrop(type: string, paths?: string[]) {
  if (!dragDropCallback) throw new Error("onDragDropEvent not registered");
  dragDropCallback({ payload: { type, paths } });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("FileDropProvider", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    dragDropCallback = null;
  });

  it("starts with isDragging false and no paths", () => {
    const { result } = renderHook(() => useFileDrop(), { wrapper });
    expect(result.current.isDragging).toBe(false);
    expect(result.current.paths).toBeNull();
  });

  it("sets isDragging true on enter event", () => {
    const { result } = renderHook(() => useFileDrop(), { wrapper });

    act(() => emitDragDrop("enter", ["/tmp/file.txt"]));

    expect(result.current.isDragging).toBe(true);
  });

  it("stores paths from enter event", () => {
    const { result } = renderHook(() => useFileDrop(), { wrapper });

    act(() => emitDragDrop("enter", ["/tmp/a.txt", "/tmp/b.txt"]));

    expect(result.current.paths).toEqual(["/tmp/a.txt", "/tmp/b.txt"]);
  });

  it("sets isDragging false on leave event", () => {
    const { result } = renderHook(() => useFileDrop(), { wrapper });

    act(() => emitDragDrop("enter", ["/tmp/file.txt"]));
    expect(result.current.isDragging).toBe(true);

    act(() => emitDragDrop("leave"));

    expect(result.current.isDragging).toBe(false);
    expect(result.current.paths).toBeNull();
  });

  it("calls registered callback on drop and resets state", () => {
    const callback = vi.fn();
    const { result } = renderHook(() => useFileDrop(), { wrapper });

    act(() => result.current.registerDropTarget(callback));
    act(() => emitDragDrop("enter", ["/tmp/file.txt"]));
    act(() => emitDragDrop("drop", ["/tmp/file.txt"]));

    expect(callback).toHaveBeenCalledWith(["/tmp/file.txt"]);
    expect(result.current.isDragging).toBe(false);
    expect(result.current.paths).toBeNull();
  });

  it("does not crash on drop when no callback is registered", () => {
    const { result } = renderHook(() => useFileDrop(), { wrapper });

    act(() => emitDragDrop("enter", ["/tmp/file.txt"]));
    // Should not throw
    act(() => emitDragDrop("drop", ["/tmp/file.txt"]));

    expect(result.current.isDragging).toBe(false);
  });

  it("unregisterDropTarget removes the callback", () => {
    const callback = vi.fn();
    const { result } = renderHook(() => useFileDrop(), { wrapper });

    act(() => result.current.registerDropTarget(callback));
    act(() => result.current.unregisterDropTarget(callback));
    act(() => emitDragDrop("enter", ["/tmp/file.txt"]));
    act(() => emitDragDrop("drop", ["/tmp/file.txt"]));

    expect(callback).not.toHaveBeenCalled();
  });

  it("only the most recently registered callback receives drops (LIFO stack)", () => {
    const first = vi.fn();
    const second = vi.fn();
    const { result } = renderHook(() => useFileDrop(), { wrapper });

    act(() => result.current.registerDropTarget(first));
    act(() => result.current.registerDropTarget(second));
    act(() => emitDragDrop("enter", ["/tmp/file.txt"]));
    act(() => emitDragDrop("drop", ["/tmp/file.txt"]));

    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledWith(["/tmp/file.txt"]);
  });

  it("unregistering top of stack restores the previous callback", () => {
    const first = vi.fn();
    const second = vi.fn();
    const { result } = renderHook(() => useFileDrop(), { wrapper });

    act(() => result.current.registerDropTarget(first));
    act(() => result.current.registerDropTarget(second));
    // Unmount the second (top of stack)
    act(() => result.current.unregisterDropTarget(second));

    act(() => emitDragDrop("enter", ["/tmp/file.txt"]));
    act(() => emitDragDrop("drop", ["/tmp/file.txt"]));

    expect(second).not.toHaveBeenCalled();
    expect(first).toHaveBeenCalledWith(["/tmp/file.txt"]);
  });

  it("unregistering a middle callback does not break the stack", () => {
    const a = vi.fn();
    const b = vi.fn();
    const c = vi.fn();
    const { result } = renderHook(() => useFileDrop(), { wrapper });

    act(() => result.current.registerDropTarget(a));
    act(() => result.current.registerDropTarget(b));
    act(() => result.current.registerDropTarget(c));
    // Remove the middle one
    act(() => result.current.unregisterDropTarget(b));

    act(() => emitDragDrop("enter", ["/tmp/file.txt"]));
    act(() => emitDragDrop("drop", ["/tmp/file.txt"]));

    // Top of stack (c) should still receive the drop
    expect(c).toHaveBeenCalledWith(["/tmp/file.txt"]);
    expect(a).not.toHaveBeenCalled();
    expect(b).not.toHaveBeenCalled();
  });

  it("unregistering all callbacks results in no-op on drop", () => {
    const a = vi.fn();
    const b = vi.fn();
    const { result } = renderHook(() => useFileDrop(), { wrapper });

    act(() => result.current.registerDropTarget(a));
    act(() => result.current.registerDropTarget(b));
    act(() => result.current.unregisterDropTarget(b));
    act(() => result.current.unregisterDropTarget(a));

    act(() => emitDragDrop("enter", ["/tmp/file.txt"]));
    act(() => emitDragDrop("drop", ["/tmp/file.txt"]));

    expect(a).not.toHaveBeenCalled();
    expect(b).not.toHaveBeenCalled();
  });
});
