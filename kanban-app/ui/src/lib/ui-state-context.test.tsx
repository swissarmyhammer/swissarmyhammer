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
    expect(result.current.inspector_stack).toEqual([]);
  });

  it("fetches initial state on mount", async () => {
    (invoke as ReturnType<typeof vi.fn>).mockResolvedValue({
      inspector_stack: ["task:abc"],
      active_view_id: "board",
      palette_open: false,
      keymap_mode: "vim",
      scope_chain: [],
    });

    const { result } = renderHook(() => useUIState(), {
      wrapper: UIStateProvider,
    });

    // Wait for the invoke to resolve
    await act(async () => {});

    expect(result.current.keymap_mode).toBe("vim");
    expect(result.current.inspector_stack).toEqual(["task:abc"]);
  });

  it("updates on ui-state-changed event", async () => {
    (invoke as ReturnType<typeof vi.fn>).mockResolvedValue({
      inspector_stack: [],
      active_view_id: "",
      palette_open: false,
      keymap_mode: "cua",
      scope_chain: [],
    });

    let eventCallback: ((event: { payload: unknown }) => void) | undefined;
    (listen as ReturnType<typeof vi.fn>).mockImplementation((_event: string, cb: (event: { payload: unknown }) => void) => {
      eventCallback = cb;
      return Promise.resolve(() => {});
    });

    const { result } = renderHook(() => useUIState(), {
      wrapper: UIStateProvider,
    });

    await act(async () => {});
    expect(result.current.keymap_mode).toBe("cua");

    // Simulate event from backend
    act(() => {
      eventCallback?.({ payload: {
        inspector_stack: [],
        active_view_id: "grid",
        palette_open: true,
        keymap_mode: "emacs",
        scope_chain: ["board:main"],
      }});
    });

    expect(result.current.keymap_mode).toBe("emacs");
    expect(result.current.active_view_id).toBe("grid");
  });
});
