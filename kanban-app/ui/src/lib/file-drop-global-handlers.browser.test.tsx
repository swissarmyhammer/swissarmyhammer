/**
 * Browser-mode tests for FileDropProvider's global document event handlers.
 *
 * Tests the MIME-type discrimination: global handlers must call
 * preventDefault for Files (Finder drags) but NOT for task card drags.
 *
 * Tests that assert task drags should NOT be intercepted will FAIL
 * until the fix is applied. This is intentional — TDD red/green.
 */

import { describe, it, expect, vi, afterEach } from "vitest";
import { render } from "vitest-browser-react";
import type { ReactNode } from "react";

// Mock Tauri APIs — FileDropProvider imports getCurrentWebview
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

import { FileDropProvider } from "./file-drop-context";

const DRAG_MIME = "application/x-swissarmyhammer-task";

function TestWrapper({ children }: { children: ReactNode }) {
  return <FileDropProvider>{children}</FileDropProvider>;
}

describe("FileDropProvider global handlers — MIME discrimination", () => {
  it("global dragover handler calls preventDefault for Files type", async () => {
    await render(
      <TestWrapper>
        <div>content</div>
      </TestWrapper>,
    );

    const dataTransfer = new DataTransfer();
    dataTransfer.items.add(new File([""], "test.txt", { type: "text/plain" }));

    const event = new DragEvent("dragover", {
      bubbles: true,
      cancelable: true,
      dataTransfer,
    });

    document.dispatchEvent(event);

    expect(event.defaultPrevented).toBe(true);
  });

  it("global drop handler calls preventDefault for Files type", async () => {
    await render(
      <TestWrapper>
        <div>content</div>
      </TestWrapper>,
    );

    const dataTransfer = new DataTransfer();
    dataTransfer.items.add(new File([""], "test.txt", { type: "text/plain" }));

    const event = new DragEvent("drop", {
      bubbles: true,
      cancelable: true,
      dataTransfer,
    });

    document.dispatchEvent(event);

    expect(event.defaultPrevented).toBe(true);
  });

  it("global dragover handler does NOT call preventDefault for task MIME type", async () => {
    await render(
      <TestWrapper>
        <div>content</div>
      </TestWrapper>,
    );

    const dataTransfer = new DataTransfer();
    dataTransfer.setData(DRAG_MIME, '{"id":"task-1"}');

    const event = new DragEvent("dragover", {
      bubbles: true,
      cancelable: true,
      dataTransfer,
    });

    document.dispatchEvent(event);

    // After the fix, task drags should NOT be intercepted
    // CURRENTLY BROKEN: the global handler calls preventDefault on ALL drags
    expect(event.defaultPrevented).toBe(false);
  });

  it("global drop handler does NOT call preventDefault for task MIME type", async () => {
    await render(
      <TestWrapper>
        <div>content</div>
      </TestWrapper>,
    );

    const dataTransfer = new DataTransfer();
    dataTransfer.setData(DRAG_MIME, '{"id":"task-1"}');

    const event = new DragEvent("drop", {
      bubbles: true,
      cancelable: true,
      dataTransfer,
    });

    document.dispatchEvent(event);

    // After the fix, task drags should NOT be intercepted
    // CURRENTLY BROKEN: the global handler calls preventDefault on ALL drags
    expect(event.defaultPrevented).toBe(false);
  });

  it("task drag bubbling through a child element gets preventDefault from document", async () => {
    await render(
      <TestWrapper>
        <div data-testid="empty-area" style={{ width: 200, height: 200 }}>
          no drop zones here
        </div>
      </TestWrapper>,
    );

    const dataTransfer = new DataTransfer();
    dataTransfer.setData(DRAG_MIME, '{"id":"task-1"}');

    const event = new DragEvent("dragover", {
      bubbles: true,
      cancelable: true,
      dataTransfer,
    });

    // Dispatch on a child — it will bubble up to document
    const area = document.querySelector('[data-testid="empty-area"]')!;
    area.dispatchEvent(event);

    // After the fix: document should NOT preventDefault for task drags
    // CURRENTLY BROKEN: global handler makes everything a valid drop target
    expect(event.defaultPrevented).toBe(false);
  });
});
