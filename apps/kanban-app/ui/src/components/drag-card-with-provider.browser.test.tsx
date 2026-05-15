/**
 * Browser-mode integration test: task card drag with FileDropProvider active.
 *
 * THE critical regression test. Renders a DropZone inside a FileDropProvider
 * and verifies that task card drops still work while file drags are blocked.
 */

import { describe, it, expect, vi } from "vitest";
import { render } from "vitest-browser-react";
import type { ReactNode } from "react";
import { DropZone } from "./drop-zone";
import type { DropZoneDescriptor } from "@/lib/drop-zones";

// Mock Tauri APIs for FileDropProvider
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: vi.fn(() => Promise.resolve(() => {})),
  }),
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve(null)),
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

import { FileDropProvider } from "@/lib/file-drop-context";

const DRAG_MIME = "application/x-swissarmyhammer-task";

function WithFileDropProvider({ children }: { children: ReactNode }) {
  return <FileDropProvider>{children}</FileDropProvider>;
}

const descriptor: DropZoneDescriptor = {
  key: "before-task-2",
  columnId: "col-1",
  beforeId: "task-2",
};

describe("DropZone + FileDropProvider integration", () => {
  it("task card drop fires onDrop when FileDropProvider is wrapping", async () => {
    const onDrop = vi.fn();

    const screen = await render(
      <WithFileDropProvider>
        <DropZone descriptor={descriptor} onDrop={onDrop} />
      </WithFileDropProvider>,
    );

    const zone = screen.container.querySelector("[data-drop-zone]")!;
    const taskPayload = JSON.stringify({ id: "task-99", entity_type: "task" });

    const dataTransfer = new DataTransfer();
    dataTransfer.setData(DRAG_MIME, taskPayload);

    zone.dispatchEvent(new DragEvent("drop", { bubbles: true, dataTransfer }));

    expect(onDrop).toHaveBeenCalledTimes(1);
    expect(onDrop).toHaveBeenCalledWith(descriptor, taskPayload);
  });

  it("file drag does NOT fire onDrop on DropZone", async () => {
    const onDrop = vi.fn();

    const screen = await render(
      <WithFileDropProvider>
        <DropZone descriptor={descriptor} onDrop={onDrop} />
      </WithFileDropProvider>,
    );

    const zone = screen.container.querySelector("[data-drop-zone]")!;

    const dataTransfer = new DataTransfer();
    dataTransfer.items.add(
      new File(["content"], "photo.png", { type: "image/png" }),
    );

    zone.dispatchEvent(new DragEvent("drop", { bubbles: true, dataTransfer }));

    // DropZone should NOT fire onDrop because getData(DRAG_MIME) is empty
    expect(onDrop).not.toHaveBeenCalled();
  });

  it("task dragover on non-DropZone area is NOT accepted by document", async () => {
    const screen = await render(
      <WithFileDropProvider>
        <div data-testid="gap" style={{ height: 100 }}>
          area between drop zones
        </div>
        <DropZone descriptor={descriptor} onDrop={vi.fn()} />
      </WithFileDropProvider>,
    );

    const gap = screen.container.querySelector('[data-testid="gap"]')!;

    const dataTransfer = new DataTransfer();
    dataTransfer.setData(DRAG_MIME, '{"id":"task-1"}');

    const event = new DragEvent("dragover", {
      bubbles: true,
      cancelable: true,
      dataTransfer,
    });

    gap.dispatchEvent(event);

    // After the fix: global handler should NOT preventDefault for task drags
    // CURRENTLY BROKEN: global handler makes everything accept drops
    expect(event.defaultPrevented).toBe(false);
  });

  it("file dragover on non-DropZone area IS prevented (no browser navigation)", async () => {
    const screen = await render(
      <WithFileDropProvider>
        <div data-testid="gap" style={{ height: 100 }}>
          area between drop zones
        </div>
      </WithFileDropProvider>,
    );

    const gap = screen.container.querySelector('[data-testid="gap"]')!;

    const dataTransfer = new DataTransfer();
    dataTransfer.items.add(
      new File(["content"], "photo.png", { type: "image/png" }),
    );

    const event = new DragEvent("dragover", {
      bubbles: true,
      cancelable: true,
      dataTransfer,
    });

    gap.dispatchEvent(event);

    // File drags should ALWAYS be prevented to stop browser navigation
    expect(event.defaultPrevented).toBe(true);
  });
});
