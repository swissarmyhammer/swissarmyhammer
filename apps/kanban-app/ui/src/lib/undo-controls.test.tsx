/**
 * Undo/Redo control state is driven by the MCP `store/undo_changed` plane.
 *
 * Contract: an `undo_changed { can_undo:false, can_redo:true }` notification
 * leaves the Undo control disabled and the Redo control enabled. This proves
 * the webview reflects undo availability from the MCP stream (the same control
 * state an external agent observes), not from a per-event refetch.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((..._args: any[]) => Promise.resolve({}));
vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...args),
}));

type ListenCallback = (event: { payload: unknown }) => void;
const listeners = new Map<string, ListenCallback[]>();
const mockListen = vi.fn((event: string, cb: ListenCallback) => {
  const cbs = listeners.get(event) ?? [];
  cbs.push(cb);
  listeners.set(event, cbs);
  return Promise.resolve(() => {});
});
vi.mock("@tauri-apps/api/event", () => ({
  listen: (...a: Parameters<typeof mockListen>) => mockListen(...a),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

import { UndoProvider, useUndoState } from "./undo-context";

const UNDO_CHANGED = "notifications/store/undo_changed";

/** Renders Undo/Redo buttons whose `disabled` mirrors the undo state. */
function UndoControls() {
  const { canUndo, canRedo } = useUndoState();
  return (
    <>
      <button data-testid="undo-btn" disabled={!canUndo}>
        Undo
      </button>
      <button data-testid="redo-btn" disabled={!canRedo}>
        Redo
      </button>
    </>
  );
}

function wrapper({ children }: { children: ReactNode }) {
  return <UndoProvider>{children}</UndoProvider>;
}

async function fireUndoChanged(payload: unknown) {
  await act(async () => {
    for (const cb of listeners.get(UNDO_CHANGED) ?? []) cb({ payload });
  });
}

async function waitForSubscription() {
  await waitFor(() => {
    expect((listeners.get(UNDO_CHANGED)?.length ?? 0) > 0).toBe(true);
  });
}

describe("Undo/Redo controls", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockListen.mockClear();
    listeners.clear();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_undo_state")
        return Promise.resolve({ can_undo: false, can_redo: false });
      return Promise.resolve({});
    });
  });

  it("undo_changed { can_undo:false, can_redo:true } disables Undo and enables Redo", async () => {
    render(<UndoControls />, { wrapper });
    await act(async () => {});
    await waitForSubscription();

    // Both controls start disabled (seed state).
    expect(screen.getByTestId("undo-btn")).toBeDisabled();
    expect(screen.getByTestId("redo-btn")).toBeDisabled();

    // Emit the MCP undo-state notification.
    await fireUndoChanged({ can_undo: false, can_redo: true });

    expect(screen.getByTestId("undo-btn")).toBeDisabled();
    expect(screen.getByTestId("redo-btn")).not.toBeDisabled();
  });

  it("undo_changed { can_undo:true, can_redo:false } enables Undo and disables Redo", async () => {
    render(<UndoControls />, { wrapper });
    await act(async () => {});
    await waitForSubscription();

    await fireUndoChanged({ can_undo: true, can_redo: false });

    expect(screen.getByTestId("undo-btn")).not.toBeDisabled();
    expect(screen.getByTestId("redo-btn")).toBeDisabled();
  });
});
