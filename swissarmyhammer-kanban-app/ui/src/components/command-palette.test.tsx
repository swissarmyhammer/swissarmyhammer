import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";

// Track getCM mock for vim insert mode tests
const handleKeyMock = vi.fn();

// Mock Tauri APIs before importing components that use them
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve("cua")),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

// Mock getCM from codemirror-vim
// NOTE: vi.mock factories are hoisted, so we can't assign to variables declared
// with let/const at module level. Instead, import getCM after mocking and use
// vi.mocked() to access the mock.
vi.mock("@replit/codemirror-vim", async () => {
  const actual = await vi.importActual<typeof import("@replit/codemirror-vim")>("@replit/codemirror-vim");
  return {
    ...actual,
    getCM: vi.fn(() => ({ state: { vim: {} }, handleKey: handleKeyMock })),
  };
});

import { invoke } from "@tauri-apps/api/core";
import { getCM } from "@replit/codemirror-vim";
import { CommandPalette } from "./command-palette";
import { CommandScopeProvider, type CommandDef } from "@/lib/command-scope";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { KeymapProvider } from "@/lib/keymap-context";

const getCMMock = vi.mocked(getCM);

const TEST_COMMANDS: CommandDef[] = [
  {
    id: "open-file",
    name: "Open File",
    keys: { vim: ":e", cua: "Ctrl+O" },
    execute: vi.fn(),
  },
  {
    id: "save-file",
    name: "Save File",
    keys: { vim: ":w", cua: "Ctrl+S" },
    execute: vi.fn(),
  },
  {
    id: "close-tab",
    name: "Close Tab",
    keys: { cua: "Ctrl+W" },
    execute: vi.fn(),
  },
];

beforeEach(() => {
  handleKeyMock.mockClear();
  getCMMock.mockClear();
  // Restore default getCM behavior
  getCMMock.mockReturnValue({ state: { vim: {} }, handleKey: handleKeyMock } as any);
});

function renderPalette(open: boolean, onClose = vi.fn()) {
  return render(
    <EntityFocusProvider>
      <KeymapProvider>
        <CommandScopeProvider commands={TEST_COMMANDS}>
          <CommandPalette open={open} onClose={onClose} />
        </CommandScopeProvider>
      </KeymapProvider>
    </EntityFocusProvider>
  );
}

describe("CommandPalette", () => {
  it("renders nothing when closed", () => {
    renderPalette(false);
    expect(screen.queryByTestId("command-palette")).toBeNull();
  });

  it("renders the palette when open", () => {
    renderPalette(true);
    expect(screen.getByTestId("command-palette")).toBeTruthy();
  });

  it("shows all commands when no filter is applied", () => {
    renderPalette(true);
    expect(screen.getByText("Open File")).toBeTruthy();
    expect(screen.getByText("Save File")).toBeTruthy();
    expect(screen.getByText("Close Tab")).toBeTruthy();
  });

  it("shows keybinding hints for the current mode", () => {
    renderPalette(true);
    // Default mode is CUA (mocked invoke returns "cua")
    expect(screen.getByText("Ctrl+O")).toBeTruthy();
    expect(screen.getByText("Ctrl+S")).toBeTruthy();
    expect(screen.getByText("Ctrl+W")).toBeTruthy();
  });

  it("calls onClose when backdrop is clicked", () => {
    const onClose = vi.fn();
    renderPalette(true, onClose);
    fireEvent.click(screen.getByTestId("command-palette-backdrop"));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("does not close when clicking inside the palette card", () => {
    const onClose = vi.fn();
    renderPalette(true, onClose);
    fireEvent.click(screen.getByTestId("command-palette"));
    expect(onClose).not.toHaveBeenCalled();
  });

  it("executes a command when its item is clicked", () => {
    const onClose = vi.fn();
    renderPalette(true, onClose);
    fireEvent.click(screen.getByText("Save File"));
    expect(TEST_COMMANDS[1].execute).toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });

  it("renders the command list with correct role", () => {
    renderPalette(true);
    const list = screen.getByTestId("command-palette-list");
    expect(list.getAttribute("role")).toBe("listbox");
  });
});

describe("CommandPalette vim insert mode", () => {
  /** Flush pending requestAnimationFrame callbacks by running them synchronously. */
  function flushRAF(count = 5) {
    for (let i = 0; i < count; i++) {
      vi.advanceTimersByTime(16); // one frame ≈ 16ms
    }
  }

  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("auto-enters insert mode when palette opens in vim mode", async () => {
    // Mock invoke to return "vim" for get_keymap_mode
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "get_keymap_mode") return Promise.resolve("vim");
      return Promise.resolve(null);
    });

    await act(async () => {
      renderPalette(true);
    });

    // Flush rAF retries so the effect can find the CM view
    await act(async () => {
      flushRAF(25);
    });

    expect(getCMMock).toHaveBeenCalled();
    expect(handleKeyMock).toHaveBeenCalledWith("i");
  });

  it("does NOT enter insert mode in CUA mode", async () => {
    // Default mock returns "cua"
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "get_keymap_mode") return Promise.resolve("cua");
      return Promise.resolve(null);
    });

    await act(async () => {
      renderPalette(true);
    });

    await act(async () => {
      flushRAF(25);
    });

    // getCM should NOT be called — the vim insert effect skips non-vim modes
    expect(handleKeyMock).not.toHaveBeenCalled();
  });

  it("retries when getCM initially returns null", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "get_keymap_mode") return Promise.resolve("vim");
      return Promise.resolve(null);
    });

    // getCM returns null for the first 3 calls, then succeeds
    let callCount = 0;
    getCMMock.mockImplementation(() => {
      callCount++;
      if (callCount <= 3) return null;
      return { state: { vim: {} }, handleKey: handleKeyMock };
    });

    await act(async () => {
      renderPalette(true);
    });

    // Flush enough frames to get past the null returns
    await act(async () => {
      flushRAF(10);
    });

    expect(callCount).toBeGreaterThan(3);
    expect(handleKeyMock).toHaveBeenCalledWith("i");
  });

  it("stops retrying after cancellation (palette closes)", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "get_keymap_mode") return Promise.resolve("vim");
      return Promise.resolve(null);
    });

    // getCM always returns null — simulates slow init
    getCMMock.mockReturnValue(null);

    let result: ReturnType<typeof render>;
    await act(async () => {
      result = renderPalette(true);
    });

    // Unmount (closes palette) — should cancel the retry loop
    await act(async () => {
      result!.unmount();
    });

    await act(async () => {
      flushRAF(30);
    });

    // handleKey should never have been called since getCM always returned null
    // and the cleanup cancelled further retries
    expect(handleKeyMock).not.toHaveBeenCalled();
  });
});
