import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import type { ReactNode } from "react";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((..._args: any[]) => Promise.resolve({}));

vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...args),
}));
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockListen = vi.fn((..._args: any[]) => Promise.resolve(() => {}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: unknown[]) => mockListen(...args),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

import { UndoProvider, useUndoState } from "./undo-context";

describe("UndoProvider", () => {
  function wrapper({ children }: { children: ReactNode }) {
    return <UndoProvider>{children}</UndoProvider>;
  }

  beforeEach(() => {
    mockInvoke.mockReset();
    mockListen.mockClear();
    // Default: get_undo_state returns both false
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_undo_state") {
        return Promise.resolve({ can_undo: false, can_redo: false });
      }
      return Promise.resolve({});
    });
  });

  it("canUndo and canRedo default to false", () => {
    const { result } = renderHook(() => useUndoState(), { wrapper });
    expect(result.current.canUndo).toBe(false);
    expect(result.current.canRedo).toBe(false);
  });

  it("undo() dispatches app.undo to backend", async () => {
    const { result } = renderHook(() => useUndoState(), { wrapper });

    await act(async () => {
      await result.current.undo();
    });

    expect(mockInvoke).toHaveBeenCalledWith("dispatch_command", {
      cmd: "app.undo",
      scopeChain: [],
    });
  });

  it("redo() dispatches app.redo to backend", async () => {
    const { result } = renderHook(() => useUndoState(), { wrapper });

    await act(async () => {
      await result.current.redo();
    });

    expect(mockInvoke).toHaveBeenCalledWith("dispatch_command", {
      cmd: "app.redo",
      scopeChain: [],
    });
  });

  it("listens for all three entity mutation events", async () => {
    renderHook(() => useUndoState(), { wrapper });

    // Wait for effects to run
    await act(async () => {});

    const listenedEvents = mockListen.mock.calls.map(
      (call: unknown[]) => call[0],
    );
    expect(listenedEvents).toContain("entity-created");
    expect(listenedEvents).toContain("entity-removed");
    expect(listenedEvents).toContain("entity-field-changed");
    expect(listenedEvents).not.toContain("entity-changed");
  });

  it("fetchUndoState error fallback returns false for both", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_undo_state") {
        return Promise.reject(new Error("not implemented"));
      }
      return Promise.resolve({});
    });

    const { result } = renderHook(() => useUndoState(), { wrapper });

    // Wait for the initial fetch to settle
    await act(async () => {});

    expect(result.current.canUndo).toBe(false);
    expect(result.current.canRedo).toBe(false);
  });
});
