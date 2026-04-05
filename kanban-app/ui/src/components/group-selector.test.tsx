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
    groupable: true,
  },
  {
    id: "priority",
    name: "Priority",
    type: { kind: "select" },
    section: "body",
    groupable: true,
  },
  {
    id: "internal_id",
    name: "internal_id",
    type: { kind: "text" },
    section: "hidden",
  },
  {
    id: "title",
    name: "Title",
    type: { kind: "text" },
    section: "header",
    groupable: false,
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

  it("only shows fields where groupable is true", () => {
    render(<GroupSelector {...defaultProps} />);

    // groupable: true fields appear
    expect(screen.getByTestId("group-field-Status")).toBeDefined();
    expect(screen.getByTestId("group-field-Priority")).toBeDefined();
    // groupable: undefined (hidden) excluded
    expect(screen.queryByTestId("group-field-internal_id")).toBeNull();
    // groupable: false excluded
    expect(screen.queryByTestId("group-field-Title")).toBeNull();
  });

  it("renders only None when no fields are groupable", () => {
    const nonGroupableFields: FieldDef[] = [
      { id: "title", name: "Title", type: { kind: "text" }, section: "header" },
      {
        id: "body",
        name: "Body",
        type: { kind: "text" },
        section: "body",
        groupable: false,
      },
    ];
    render(<GroupSelector {...defaultProps} fields={nonGroupableFields} />);

    expect(screen.getByTestId("group-none")).toBeDefined();
    // No field buttons rendered
    expect(screen.queryByTestId("group-field-Title")).toBeNull();
    expect(screen.queryByTestId("group-field-Body")).toBeNull();
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

  it("does not render a Clear button regardless of group state", () => {
    const { rerender } = render(
      <GroupSelector {...defaultProps} group={undefined} />,
    );
    expect(screen.queryByLabelText("Clear group")).toBeNull();

    rerender(<GroupSelector {...defaultProps} group="Status" />);
    expect(screen.queryByLabelText("Clear group")).toBeNull();
  });
});
