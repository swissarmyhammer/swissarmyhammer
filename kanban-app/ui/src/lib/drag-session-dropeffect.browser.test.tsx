/**
 * Browser-mode tests for cross-board drag session dropEffect behavior.
 *
 * The critical regression: when FileDropProvider's global handler calls
 * preventDefault on ALL dragover events, the document becomes a valid
 * drop target. This causes dropEffect to never be "none", breaking
 * the cross-board cancel logic.
 *
 * These tests verify that the document does NOT accept task card dragover
 * events while FileDropProvider is active.
 */

import { describe, it, expect, vi } from "vitest";
import { render } from "vitest-browser-react";

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

import { FileDropProvider } from "./file-drop-context";

const DRAG_MIME = "application/x-swissarmyhammer-task";

describe("Cross-board drag — dropEffect detection", () => {
  it("task dragover on non-target area does NOT get preventDefault when FileDropProvider wraps", async () => {
    await render(
      <FileDropProvider>
        <div data-testid="no-drop-zone" style={{ width: 200, height: 200 }}>
          This area should NOT accept drops
        </div>
      </FileDropProvider>,
    );

    const noDropZone = document.querySelector('[data-testid="no-drop-zone"]')!;

    const dataTransfer = new DataTransfer();
    dataTransfer.setData(DRAG_MIME, '{"id":"task-1"}');

    const event = new DragEvent("dragover", {
      bubbles: true,
      cancelable: true,
      dataTransfer,
    });

    noDropZone.dispatchEvent(event);

    // After fix: document's global handler should NOT preventDefault for
    // task drags, so this area is not a valid drop target
    // CURRENTLY BROKEN: global handler makes everything accept drops
    expect(event.defaultPrevented).toBe(false);
  });

  it("file dragover on non-target area IS prevented (browser safety)", async () => {
    await render(
      <FileDropProvider>
        <div data-testid="no-drop-zone" style={{ width: 200, height: 200 }}>
          This area should NOT accept drops
        </div>
      </FileDropProvider>,
    );

    const noDropZone = document.querySelector('[data-testid="no-drop-zone"]')!;

    const dataTransfer = new DataTransfer();
    dataTransfer.items.add(
      new File(["content"], "photo.png", { type: "image/png" }),
    );

    const event = new DragEvent("dragover", {
      bubbles: true,
      cancelable: true,
      dataTransfer,
    });

    noDropZone.dispatchEvent(event);

    // File drags should always be prevented to stop browser navigation
    expect(event.defaultPrevented).toBe(true);
  });

  it("task drop on document is NOT prevented (allows dropEffect none)", async () => {
    await render(
      <FileDropProvider>
        <div>content</div>
      </FileDropProvider>,
    );

    const dataTransfer = new DataTransfer();
    dataTransfer.setData(DRAG_MIME, '{"id":"task-1"}');

    const event = new DragEvent("drop", {
      bubbles: true,
      cancelable: true,
      dataTransfer,
    });

    document.dispatchEvent(event);

    // After fix: document should NOT swallow task drops
    // CURRENTLY BROKEN: global handler prevents all drops
    expect(event.defaultPrevented).toBe(false);
  });
});
