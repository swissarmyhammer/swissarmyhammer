/**
 * Browser-mode tests for FileDropProvider using HTML5 drag events.
 *
 * These run in real Chromium where DataTransfer and DragEvent work natively.
 * Tests verify the full file drop pipeline: dragenter → isDragging →
 * drop → save_dropped_file invoke → callback with temp paths.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render } from "vitest-browser-react";
import { useEffect, useRef } from "react";

// Mock invoke for save_dropped_file
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve("/tmp/kanban-drops/test-file.txt")),
}));

import { FileDropProvider, useFileDrop, type DropCallback } from "./file-drop-context";

/** Test component that exposes FileDropProvider state and registration. */
function DropProbe({
  onState,
  callback,
}: {
  onState: (state: { isDragging: boolean; paths: string[] | null }) => void;
  callback?: DropCallback;
}) {
  const { isDragging, paths, registerDropTarget, unregisterDropTarget } =
    useFileDrop();

  onState({ isDragging, paths });

  const cbRef = useRef(callback);
  cbRef.current = callback;
  useEffect(() => {
    if (!cbRef.current) return;
    const cb = cbRef.current;
    registerDropTarget(cb);
    return () => unregisterDropTarget(cb);
  }, [registerDropTarget, unregisterDropTarget]);

  return <div data-testid="probe">probe</div>;
}

function emitFileDragEnter(filenames: string[] = ["test.txt"]) {
  const dt = new DataTransfer();
  for (const name of filenames) {
    dt.items.add(new File(["content"], name, { type: "text/plain" }));
  }
  document.dispatchEvent(
    new DragEvent("dragenter", { bubbles: true, dataTransfer: dt }),
  );
}

function emitFileDragLeave() {
  const dt = new DataTransfer();
  dt.items.add(new File([""], "x", { type: "text/plain" }));
  document.dispatchEvent(
    new DragEvent("dragleave", { bubbles: true, dataTransfer: dt }),
  );
}

function emitFileDrop(filenames: string[] = ["test.txt"]) {
  const dt = new DataTransfer();
  for (const name of filenames) {
    dt.items.add(new File(["content"], name, { type: "text/plain" }));
  }
  document.dispatchEvent(
    new DragEvent("drop", { bubbles: true, cancelable: true, dataTransfer: dt }),
  );
}

const tick = (ms = 50) => new Promise((r) => setTimeout(r, ms));

describe("FileDropProvider — HTML5 drag events", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("starts with isDragging false", async () => {
    let state = { isDragging: false, paths: null as string[] | null };
    await render(
      <FileDropProvider>
        <DropProbe onState={(s) => (state = s)} />
      </FileDropProvider>,
    );
    expect(state.isDragging).toBe(false);
    expect(state.paths).toBeNull();
  });

  it("sets isDragging true on file dragenter", async () => {
    let state = { isDragging: false, paths: null as string[] | null };
    await render(
      <FileDropProvider>
        <DropProbe onState={(s) => (state = s)} />
      </FileDropProvider>,
    );

    emitFileDragEnter(["photo.png"]);
    await tick();
    expect(state.isDragging).toBe(true);
  });

  it("stores filenames from dragenter", async () => {
    let state = { isDragging: false, paths: null as string[] | null };
    await render(
      <FileDropProvider>
        <DropProbe onState={(s) => (state = s)} />
      </FileDropProvider>,
    );

    emitFileDragEnter(["a.txt", "b.txt"]);
    await tick();
    expect(state.paths).toEqual(["a.txt", "b.txt"]);
  });

  it("sets isDragging false on dragleave", async () => {
    let state = { isDragging: false, paths: null as string[] | null };
    await render(
      <FileDropProvider>
        <DropProbe onState={(s) => (state = s)} />
      </FileDropProvider>,
    );

    emitFileDragEnter(["file.txt"]);
    await tick();
    expect(state.isDragging).toBe(true);

    emitFileDragLeave();
    await tick();
    expect(state.isDragging).toBe(false);
    expect(state.paths).toBeNull();
  });

  it("calls registered callback on drop with temp paths", async () => {
    const callback = vi.fn();
    await render(
      <FileDropProvider>
        <DropProbe onState={() => {}} callback={callback} />
      </FileDropProvider>,
    );

    emitFileDragEnter(["file.txt"]);
    emitFileDrop(["file.txt"]);
    await tick(100);

    expect(callback).toHaveBeenCalledWith(["/tmp/kanban-drops/test-file.txt"]);
  });

  it("does not crash on drop when no callback is registered", async () => {
    await render(
      <FileDropProvider>
        <DropProbe onState={() => {}} />
      </FileDropProvider>,
    );

    emitFileDragEnter(["file.txt"]);
    emitFileDrop(["file.txt"]);
    await tick(100);
    expect(true).toBe(true);
  });

  it("does not fire callback for task card drops", async () => {
    const callback = vi.fn();
    await render(
      <FileDropProvider>
        <DropProbe onState={() => {}} callback={callback} />
      </FileDropProvider>,
    );

    const dt = new DataTransfer();
    dt.setData("application/x-swissarmyhammer-task", '{"id":"task-1"}');
    document.dispatchEvent(
      new DragEvent("drop", { bubbles: true, cancelable: true, dataTransfer: dt }),
    );
    await tick(100);

    expect(callback).not.toHaveBeenCalled();
  });
});
