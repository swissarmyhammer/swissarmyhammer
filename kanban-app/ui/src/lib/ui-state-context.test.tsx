import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { UIStateProvider, useUIState } from "./ui-state-context";

// Mock Tauri
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Minimal valid UIStateSnapshot matching the new backend shape. */
function makeState(overrides: Record<string, unknown> = {}) {
  return {
    palette_open: false,
    keymap_mode: "cua",
    scope_chain: [],
    open_boards: [],
    window_boards: {},
    windows: {},
    recent_boards: [],
    ...overrides,
  };
}

describe("useUIState", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("returns default state before fetch completes", () => {
    (invoke as ReturnType<typeof vi.fn>).mockReturnValue(new Promise(() => {})); // never resolves
    const { result } = renderHook(() => useUIState(), {
      wrapper: UIStateProvider,
    });
    expect(result.current.keymap_mode).toBe("cua");
    expect(result.current.palette_open).toBe(false);
  });

  it("fetches initial state on mount", async () => {
    (invoke as ReturnType<typeof vi.fn>).mockResolvedValue(
      makeState({
        keymap_mode: "vim",
        windows: {
          main: { inspector_stack: ["task:abc"], active_view_id: "board" },
        },
      }),
    );

    const { result } = renderHook(() => useUIState(), {
      wrapper: UIStateProvider,
    });

    // Wait for the invoke to resolve
    await act(async () => {});

    expect(result.current.keymap_mode).toBe("vim");
    expect(result.current.windows["main"]?.inspector_stack).toEqual([
      "task:abc",
    ]);
    expect(result.current.windows["main"]?.active_view_id).toBe("board");
  });

  it("updates on ui-state-changed event", async () => {
    (invoke as ReturnType<typeof vi.fn>).mockResolvedValue(makeState());

    let eventCallback: ((event: { payload: unknown }) => void) | undefined;
    (listen as ReturnType<typeof vi.fn>).mockImplementation(
      (_event: string, cb: (event: { payload: unknown }) => void) => {
        eventCallback = cb;
        return Promise.resolve(() => {});
      },
    );

    const { result } = renderHook(() => useUIState(), {
      wrapper: UIStateProvider,
    });

    await act(async () => {});
    expect(result.current.keymap_mode).toBe("cua");

    // Simulate event from backend with per-window active_view_id
    act(() => {
      eventCallback?.({
        payload: makeState({
          palette_open: true,
          keymap_mode: "emacs",
          scope_chain: ["board:main"],
          windows: { main: { inspector_stack: [], active_view_id: "grid" } },
        }),
      });
    });

    expect(result.current.keymap_mode).toBe("emacs");
    expect(result.current.windows["main"]?.active_view_id).toBe("grid");
  });
});
