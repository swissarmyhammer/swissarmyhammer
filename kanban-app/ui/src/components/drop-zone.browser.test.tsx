/**
 * Browser-mode drag-and-drop tests for DropZone.
 *
 * These run in real Chromium via vitest-browser-react. In browser mode,
 * React synthetic events only fire from real user interactions, not from
 * programmatic dispatchEvent. So we test:
 *
 * 1. Structural correctness (data attributes, rendering)
 * 2. Event propagation behavior (stopPropagation on native listeners)
 * 3. The core integration: DropZone inside FileDropProvider
 *
 * For handler logic (onDrop, dropEffect), see the jsdom unit tests
 * in drop-zone.test.tsx which can use fireEvent to trigger React handlers.
 */

import { describe, it, expect, vi } from "vitest";
import { render } from "vitest-browser-react";
import { DropZone } from "./drop-zone";
import type { DropZoneDescriptor } from "@/lib/drop-zones";

const DRAG_MIME = "application/x-swissarmyhammer-task";

function makeDescriptor(
  overrides: Partial<DropZoneDescriptor> = {},
): DropZoneDescriptor {
  return {
    key: "before-task-2",
    columnId: "col-1",
    beforeId: "task-2",
    ...overrides,
  };
}

describe("DropZone — browser drag events", () => {
  it("renders with correct data attributes", async () => {
    const screen = await render(
      <DropZone descriptor={makeDescriptor()} onDrop={vi.fn()} />,
    );

    const zone = screen.container.querySelector("[data-drop-zone]");
    expect(zone).not.toBeNull();
    expect(zone!.getAttribute("data-drop-before")).toBe("task-2");
  });

  it("renders empty-column variant with data-drop-empty", async () => {
    const descriptor = makeDescriptor({
      key: "empty",
      beforeId: undefined,
    });
    const screen = await render(
      <DropZone
        descriptor={descriptor}
        variant="empty-column"
        onDrop={vi.fn()}
      />,
    );

    const zone = screen.container.querySelector("[data-drop-empty]");
    expect(zone).not.toBeNull();
    expect(zone!.hasAttribute("data-drop-zone")).toBe(true);
  });

  it("has onDragOver handler that calls stopPropagation (prevents document bubbling)", async () => {
    // Register a document-level listener BEFORE rendering
    const documentDragOver = vi.fn();
    document.addEventListener("dragover", documentDragOver);

    try {
      const screen = await render(
        <DropZone descriptor={makeDescriptor()} onDrop={vi.fn()} />,
      );

      const zone = screen.container.querySelector("[data-drop-zone]")!;

      // In real Chromium, dispatchEvent fires native handlers on the
      // element and then bubbles. React's delegated handler on the root
      // catches it and dispatches synthetic events. The React handler
      // calls stopPropagation which prevents bubbling to document.
      const dt = new DataTransfer();
      dt.setData(DRAG_MIME, "{}");

      zone.dispatchEvent(
        new DragEvent("dragover", {
          bubbles: true,
          cancelable: true,
          dataTransfer: dt,
        }),
      );

      // stopPropagation in DropZone's handler prevents this from reaching document
      expect(documentDragOver).not.toHaveBeenCalled();
    } finally {
      document.removeEventListener("dragover", documentDragOver);
    }
  });

  it("does NOT fire onDrop when drop has no task MIME data", async () => {
    const onDrop = vi.fn();
    const screen = await render(
      <DropZone descriptor={makeDescriptor()} onDrop={onDrop} />,
    );

    const zone = screen.container.querySelector("[data-drop-zone]")!;

    // Drop with empty DataTransfer (no task MIME)
    const dataTransfer = new DataTransfer();
    zone.dispatchEvent(new DragEvent("drop", { bubbles: true, dataTransfer }));

    expect(onDrop).not.toHaveBeenCalled();
  });

  it("fires onDrop when drop contains task MIME data", async () => {
    const onDrop = vi.fn();
    const descriptor = makeDescriptor();
    const screen = await render(
      <DropZone descriptor={descriptor} onDrop={onDrop} />,
    );

    const zone = screen.container.querySelector("[data-drop-zone]")!;
    const taskPayload = JSON.stringify({ id: "task-99", entity_type: "task" });
    const dataTransfer = new DataTransfer();
    dataTransfer.setData(DRAG_MIME, taskPayload);

    zone.dispatchEvent(new DragEvent("drop", { bubbles: true, dataTransfer }));

    expect(onDrop).toHaveBeenCalledTimes(1);
    expect(onDrop).toHaveBeenCalledWith(descriptor, taskPayload);
  });

  it("inert spacer does not respond to drops", async () => {
    const onDrop = vi.fn();
    const descriptor = makeDescriptor({ beforeId: "task-2" });

    const screen = await render(
      <DropZone descriptor={descriptor} dragTaskId="task-2" onDrop={onDrop} />,
    );

    const zone = screen.container.querySelector("[data-drop-zone]")!;
    const dt = new DataTransfer();
    dt.setData(DRAG_MIME, '{"id":"task-2"}');

    zone.dispatchEvent(
      new DragEvent("drop", { bubbles: true, dataTransfer: dt }),
    );

    expect(onDrop).not.toHaveBeenCalled();
  });

  it("empty-column variant accepts task drops", async () => {
    const onDrop = vi.fn();
    const descriptor = makeDescriptor({
      key: "empty",
      beforeId: undefined,
      afterId: undefined,
    });

    const screen = await render(
      <DropZone
        descriptor={descriptor}
        variant="empty-column"
        onDrop={onDrop}
      />,
    );

    const zone = screen.container.querySelector("[data-drop-empty]")!;
    const taskPayload = JSON.stringify({ id: "task-1" });
    const dt = new DataTransfer();
    dt.setData(DRAG_MIME, taskPayload);

    zone.dispatchEvent(
      new DragEvent("drop", { bubbles: true, dataTransfer: dt }),
    );

    expect(onDrop).toHaveBeenCalledWith(descriptor, taskPayload);
  });
});
