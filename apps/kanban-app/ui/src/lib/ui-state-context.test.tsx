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
    keymap_mode: "cua",
    scope_chain: [],
    open_boards: [],
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
    // palette_open lives in per-window state; default has no windows
    expect(result.current.windows).toEqual({});
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

    // Simulate event from backend: the wire format is `{ kind, state }`
    // where `state` is the full UIState snapshot.
    act(() => {
      eventCallback?.({
        payload: {
          kind: "keymap_mode",
          state: makeState({
            palette_open: true,
            keymap_mode: "emacs",
            scope_chain: ["board:main"],
            windows: { main: { inspector_stack: [], active_view_id: "grid" } },
          }),
        },
      });
    });

    expect(result.current.keymap_mode).toBe("emacs");
    expect(result.current.windows["main"]?.active_view_id).toBe("grid");
  });

  /**
   * Helper: mount UIStateProvider with a capturable event listener and
   * resolved initial state, then return the hook result plus an `emit`
   * function that fires a fake `ui-state-changed` payload.
   */
  async function mountWithEventListener() {
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
    // Flush initial-fetch effect
    await act(async () => {});
    const emit = (payload: unknown) => {
      act(() => {
        eventCallback?.({ payload });
      });
    };
    return { result, emit };
  }

  // ─── Discriminator-aware listener tests ──────────────────────────────
  //
  // These guard the per-keystroke render-storm fix: `ui.setFocus` returns
  // `UIStateChange::ScopeChain(...)` on every arrow key, which the backend
  // emits as `{ kind: "scope_chain", ... }`. The `UIStateProvider` must
  // ignore that kind so `useUIState()` stays reference-stable — otherwise
  // every focus move cascades re-renders through every consumer.

  it("scope_chain events do not change useUIState() identity", async () => {
    const { result, emit } = await mountWithEventListener();
    const beforeRef = result.current;

    emit({
      kind: "scope_chain",
      state: makeState({
        scope_chain: ["board:main", "column:todo", "task:01ABC"],
      }),
    });

    // Reference-stable: React skipped re-render of useUIState() consumers.
    expect(result.current).toBe(beforeRef);
  });

  it("palette_open events update state", async () => {
    const { result, emit } = await mountWithEventListener();
    const beforeRef = result.current;

    emit({
      kind: "palette_open",
      state: makeState({
        windows: {
          main: {
            inspector_stack: [],
            active_view_id: "",
            active_perspective_id: "",
            palette_open: true,
            palette_mode: "command",
            app_mode: "normal",
            board_path: "",
          },
        },
      }),
    });

    expect(result.current).not.toBe(beforeRef);
    expect(result.current.windows["main"]?.palette_open).toBe(true);
  });

  it("board_switch events update state", async () => {
    const { result, emit } = await mountWithEventListener();
    const beforeRef = result.current;

    emit({
      kind: "board_switch",
      state: makeState({ open_boards: ["/boards/new-board"] }),
    });

    expect(result.current).not.toBe(beforeRef);
    expect(result.current.open_boards).toEqual(["/boards/new-board"]);
  });

  it("keymap_mode, inspector_stack, active_view, active_perspective, app_mode, board_close all propagate", async () => {
    // One test covers the five "pass-through" kinds at once — each one
    // updates state, identical in behavior to palette_open / board_switch.
    const { result, emit } = await mountWithEventListener();
    const kinds = [
      "keymap_mode",
      "inspector_stack",
      "active_view",
      "active_perspective",
      "app_mode",
      "board_close",
    ] as const;
    for (const kind of kinds) {
      const beforeRef = result.current;
      emit({
        kind,
        state: makeState({ keymap_mode: `mode-for-${kind}` }),
      });
      expect(
        result.current,
        `${kind} should produce a new state reference`,
      ).not.toBe(beforeRef);
      expect(result.current.keymap_mode).toBe(`mode-for-${kind}`);
    }
  });
});
