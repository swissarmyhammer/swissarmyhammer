import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

// Mock Tauri APIs before importing components that use them
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve("cua")),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

import { CommandPalette } from "./command-palette";
import { CommandScopeProvider, type CommandDef } from "@/lib/command-scope";
import { KeymapProvider } from "@/lib/keymap-context";

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

function renderPalette(open: boolean, onClose = vi.fn()) {
  return render(
    <KeymapProvider>
      <CommandScopeProvider commands={TEST_COMMANDS}>
        <CommandPalette open={open} onClose={onClose} />
      </CommandScopeProvider>
    </KeymapProvider>
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
