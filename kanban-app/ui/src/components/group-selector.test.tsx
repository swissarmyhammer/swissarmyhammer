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

import { GroupSelector } from "./group-selector";
import type { FieldDef } from "@/types/kanban";

/** Minimal field definitions for testing. */
const testFields: FieldDef[] = [
  {
    id: "status",
    name: "Status",
    type: { kind: "select" },
    section: "body",
  },
  {
    id: "priority",
    name: "Priority",
    type: { kind: "select" },
    section: "body",
  },
  {
    id: "internal_id",
    name: "internal_id",
    type: { kind: "text" },
    section: "hidden",
  },
];

describe("GroupSelector", () => {
  const defaultProps = {
    group: undefined as string | undefined,
    perspectiveId: "p1",
    fields: testFields,
    onClose: vi.fn(),
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders the group selector with label", () => {
    render(<GroupSelector {...defaultProps} />);

    expect(screen.getByText("Group By")).toBeDefined();
    expect(screen.getByTestId("group-selector")).toBeDefined();
  });

  it("renders None option and groupable fields", () => {
    render(<GroupSelector {...defaultProps} />);

    expect(screen.getByTestId("group-none")).toBeDefined();
    expect(screen.getByTestId("group-field-Status")).toBeDefined();
    expect(screen.getByTestId("group-field-Priority")).toBeDefined();
  });

  it("excludes hidden fields from the list", () => {
    render(<GroupSelector {...defaultProps} />);

    expect(screen.queryByTestId("group-field-internal_id")).toBeNull();
  });

  it("dispatches perspective.group when a field is selected", () => {
    const onClose = vi.fn();
    render(<GroupSelector {...defaultProps} onClose={onClose} />);

    fireEvent.click(screen.getByTestId("group-field-Status"));

    expect(mockInvoke).toHaveBeenCalledWith(
      "dispatch_command",
      expect.objectContaining({
        cmd: "perspective.group",
        args: { group: "Status", perspective_id: "p1" },
      }),
    );
    expect(onClose).toHaveBeenCalled();
  });

  it("dispatches perspective.clearGroup when None is selected", () => {
    const onClose = vi.fn();
    render(
      <GroupSelector {...defaultProps} group="Status" onClose={onClose} />,
    );

    fireEvent.click(screen.getByTestId("group-none"));

    expect(mockInvoke).toHaveBeenCalledWith(
      "dispatch_command",
      expect.objectContaining({
        cmd: "perspective.clearGroup",
        args: { perspective_id: "p1" },
      }),
    );
    expect(onClose).toHaveBeenCalled();
  });

  it("does not show clear button when group is empty", () => {
    render(<GroupSelector {...defaultProps} group={undefined} />);

    expect(screen.queryByLabelText("Clear group")).toBeNull();
  });

  it("shows clear button when group is active", () => {
    render(<GroupSelector {...defaultProps} group="Status" />);

    expect(screen.getByLabelText("Clear group")).toBeDefined();
  });

  it("dispatches clearGroup when clear button is clicked", () => {
    const onClose = vi.fn();
    render(
      <GroupSelector {...defaultProps} group="Priority" onClose={onClose} />,
    );

    fireEvent.click(screen.getByLabelText("Clear group"));

    expect(mockInvoke).toHaveBeenCalledWith(
      "dispatch_command",
      expect.objectContaining({
        cmd: "perspective.clearGroup",
        args: { perspective_id: "p1" },
      }),
    );
    expect(onClose).toHaveBeenCalled();
  });
});
