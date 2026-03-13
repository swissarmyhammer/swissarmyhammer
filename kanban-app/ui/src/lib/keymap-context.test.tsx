import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { KeymapProvider, useKeymap } from "./keymap-context";

// Mock Tauri APIs
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

import { invoke } from "@tauri-apps/api/core";
const mockInvoke = vi.mocked(invoke);

function TestConsumer() {
  const { mode, setMode } = useKeymap();
  return (
    <div>
      <span data-testid="mode">{mode}</span>
      <button onClick={() => setMode("vim")}>Set Vim</button>
      <button onClick={() => setMode("emacs")}>Set Emacs</button>
      <button onClick={() => setMode("cua")}>Set CUA</button>
    </div>
  );
}

describe("KeymapContext", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    // Default: get_keymap_mode returns "cua"
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_keymap_mode") return Promise.resolve("cua");
      return Promise.resolve({});
    });
  });

  it("defaults to cua", () => {
    render(
      <KeymapProvider>
        <TestConsumer />
      </KeymapProvider>
    );
    expect(screen.getByTestId("mode").textContent).toBe("cua");
  });

  it("reads stored mode from backend on mount", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_keymap_mode") return Promise.resolve("vim");
      return Promise.resolve({});
    });
    render(
      <KeymapProvider>
        <TestConsumer />
      </KeymapProvider>
    );
    await waitFor(() => {
      expect(screen.getByTestId("mode").textContent).toBe("vim");
    });
  });

  it("updates mode and calls set_keymap_mode command", () => {
    render(
      <KeymapProvider>
        <TestConsumer />
      </KeymapProvider>
    );
    fireEvent.click(screen.getByText("Set Vim"));
    expect(screen.getByTestId("mode").textContent).toBe("vim");
    expect(mockInvoke).toHaveBeenCalledWith("set_keymap_mode", { mode: "vim" });
  });

  it("can cycle through all modes", () => {
    render(
      <KeymapProvider>
        <TestConsumer />
      </KeymapProvider>
    );
    fireEvent.click(screen.getByText("Set Emacs"));
    expect(screen.getByTestId("mode").textContent).toBe("emacs");

    fireEvent.click(screen.getByText("Set Vim"));
    expect(screen.getByTestId("mode").textContent).toBe("vim");

    fireEvent.click(screen.getByText("Set CUA"));
    expect(screen.getByTestId("mode").textContent).toBe("cua");
  });
});
