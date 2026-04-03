import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

// Mock Tauri APIs before importing any modules that use them.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((..._args: any[]) => Promise.resolve(null));
vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

// Mock UIState context for keymap mode.
vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => ({ keymap_mode: "cua" }),
}));

// Mock backendDispatch to capture commands.
const mockBackendDispatch = vi.fn(() => Promise.resolve(null));
vi.mock("@/lib/command-scope", async (importOriginal) => {
  const original = await importOriginal<typeof import("@/lib/command-scope")>();
  return {
    ...original,
    backendDispatch: (...args: unknown[]) => mockBackendDispatch(...args),
  };
});

import { FilterEditor } from "./filter-editor";

describe("FilterEditor", () => {
  const defaultProps = {
    filter: "",
    perspectiveId: "p1",
    onClose: vi.fn(),
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders the filter editor with label", () => {
    render(<FilterEditor {...defaultProps} />);

    expect(screen.getByText("Filter Expression")).toBeDefined();
    expect(screen.getByTestId("filter-editor")).toBeDefined();
  });

  it("renders help text", () => {
    render(<FilterEditor {...defaultProps} />);

    expect(
      screen.getByText("Enter to save, Escape to cancel"),
    ).toBeDefined();
  });

  it("does not show clear button when filter is empty", () => {
    render(<FilterEditor {...defaultProps} filter="" />);

    expect(screen.queryByLabelText("Clear filter")).toBeNull();
  });

  it("shows clear button when filter is non-empty", () => {
    render(<FilterEditor {...defaultProps} filter='Status !== "Done"' />);

    expect(screen.getByLabelText("Clear filter")).toBeDefined();
  });

  it("dispatches clearFilter command when clear button is clicked", () => {
    const onClose = vi.fn();
    render(
      <FilterEditor
        filter='Status !== "Done"'
        perspectiveId="p1"
        onClose={onClose}
      />,
    );

    fireEvent.click(screen.getByLabelText("Clear filter"));

    expect(mockBackendDispatch).toHaveBeenCalledWith({
      cmd: "perspective.clearFilter",
      args: { perspective_id: "p1" },
    });
    expect(onClose).toHaveBeenCalled();
  });

  it("calls onClose when clear is clicked", () => {
    const onClose = vi.fn();
    render(
      <FilterEditor
        filter='Status !== "Done"'
        perspectiveId="p1"
        onClose={onClose}
      />,
    );

    fireEvent.click(screen.getByLabelText("Clear filter"));
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
