import { describe, it, expect, vi } from "vitest";
import { render, fireEvent } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve("ok")),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

import { DropZone } from "./drop-zone";
import type { DropZoneDescriptor } from "@/lib/drop-zones";

describe("DropZone", () => {
  const baseDescriptor: DropZoneDescriptor = {
    key: "before-task-2",
    boardPath: "/boards/test",
    columnId: "col-1",
    beforeId: "task-2",
  };

  it("renders data-drop-zone attribute", () => {
    render(<DropZone descriptor={baseDescriptor} onDrop={vi.fn()} />);
    const zone = document.querySelector("[data-drop-zone]");
    expect(zone).toBeTruthy();
  });

  it("renders data-drop-before when descriptor has beforeId", () => {
    render(<DropZone descriptor={baseDescriptor} onDrop={vi.fn()} />);
    const zone = document.querySelector('[data-drop-before="task-2"]');
    expect(zone).toBeTruthy();
  });

  it("renders data-drop-after when descriptor has afterId", () => {
    const descriptor: DropZoneDescriptor = {
      key: "after-task-3",
      boardPath: "/boards/test",
      columnId: "col-1",
      afterId: "task-3",
    };
    render(<DropZone descriptor={descriptor} onDrop={vi.fn()} />);
    const zone = document.querySelector('[data-drop-after="task-3"]');
    expect(zone).toBeTruthy();
  });

  it("renders data-drop-empty for empty-column variant", () => {
    const descriptor: DropZoneDescriptor = {
      key: "empty",
      boardPath: "/boards/test",
      columnId: "col-1",
    };
    render(
      <DropZone
        descriptor={descriptor}
        variant="empty-column"
        onDrop={vi.fn()}
      />,
    );
    const zone = document.querySelector("[data-drop-empty]");
    expect(zone).toBeTruthy();
    // Should also have data-drop-zone
    expect(zone?.hasAttribute("data-drop-zone")).toBe(true);
  });

  it("fires onDrop with descriptor when drop event occurs", () => {
    const onDrop = vi.fn();
    render(<DropZone descriptor={baseDescriptor} onDrop={onDrop} />);
    const zone = document.querySelector("[data-drop-zone]")!;

    const taskPayload = JSON.stringify({ id: "task-99", entity_type: "task" });
    fireEvent.drop(zone, {
      dataTransfer: {
        getData: () => taskPayload,
      },
    });

    expect(onDrop).toHaveBeenCalledTimes(1);
    expect(onDrop).toHaveBeenCalledWith(baseDescriptor, taskPayload);
  });

  it("empty-column zone fires onDrop (no before/after in descriptor)", () => {
    const emptyDescriptor: DropZoneDescriptor = {
      key: "empty",
      boardPath: "/boards/test",
      columnId: "col-1",
    };
    const onDrop = vi.fn();
    render(
      <DropZone
        descriptor={emptyDescriptor}
        variant="empty-column"
        onDrop={onDrop}
      />,
    );
    const zone = document.querySelector("[data-drop-empty]")!;

    const taskPayload = JSON.stringify({ id: "task-42", entity_type: "task" });
    fireEvent.drop(zone, {
      dataTransfer: {
        getData: () => taskPayload,
      },
    });

    expect(onDrop).toHaveBeenCalledTimes(1);
    expect(onDrop).toHaveBeenCalledWith(emptyDescriptor, taskPayload);
    // Verify descriptor has no before/after
    expect(emptyDescriptor.beforeId).toBeUndefined();
    expect(emptyDescriptor.afterId).toBeUndefined();
  });

  it("renders inert spacer when dragTaskId matches beforeId", () => {
    const onDrop = vi.fn();
    const { container } = render(
      <DropZone
        descriptor={baseDescriptor}
        dragTaskId="task-2"
        onDrop={onDrop}
      />,
    );
    // Zone still renders (preserves layout) but is inert
    const zone = container.querySelector("[data-drop-zone]");
    expect(zone).not.toBeNull();
    // Dropping on the inert spacer does nothing
    fireEvent.drop(zone!);
    expect(onDrop).not.toHaveBeenCalled();
  });

  it("renders inert spacer when dragTaskId matches afterId", () => {
    const onDrop = vi.fn();
    const descriptor: DropZoneDescriptor = {
      key: "after-task-3",
      boardPath: "/boards/test",
      columnId: "col-1",
      afterId: "task-3",
    };
    const { container } = render(
      <DropZone descriptor={descriptor} dragTaskId="task-3" onDrop={onDrop} />,
    );
    const zone = container.querySelector("[data-drop-zone]");
    expect(zone).not.toBeNull();
    fireEvent.drop(zone!);
    expect(onDrop).not.toHaveBeenCalled();
  });
});
