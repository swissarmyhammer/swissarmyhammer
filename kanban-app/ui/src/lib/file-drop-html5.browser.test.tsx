/**
 * Browser-mode test: verify HTML5 dataTransfer.files works for file drops.
 *
 * This tests whether we can use the standard HTML5 drop API to receive
 * file information instead of Tauri's onDragDropEvent. If dataTransfer.files
 * contains real File objects after a drop, we can rewrite FileDropProvider
 * to use HTML5 drag events and eliminate the Tauri native handler conflict.
 */

import { describe, it, expect, vi } from "vitest";
import { render } from "vitest-browser-react";
import { useState, useCallback } from "react";

/**
 * Minimal drop target that uses HTML5 drag events (not Tauri native).
 * Reports what it receives in dataTransfer.files on drop.
 */
function HTML5DropTarget({
  onFilesReceived,
}: {
  onFilesReceived: (
    files: { name: string; size: number; type: string }[],
  ) => void;
}) {
  const [isOver, setIsOver] = useState(false);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsOver(true);
  }, []);

  const handleDragLeave = useCallback(() => {
    setIsOver(false);
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      setIsOver(false);

      const files = Array.from(e.dataTransfer.files).map((f) => ({
        name: f.name,
        size: f.size,
        type: f.type,
      }));
      onFilesReceived(files);
    },
    [onFilesReceived],
  );

  return (
    <div
      data-testid="drop-target"
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
      style={{
        width: 300,
        height: 200,
        border: isOver ? "2px solid blue" : "2px dashed gray",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      {isOver ? "Drop here" : "Drag files here"}
    </div>
  );
}

describe("HTML5 file drop — dataTransfer.files", () => {
  it("receives File objects from dataTransfer.files on drop", async () => {
    const onFilesReceived = vi.fn();
    const screen = await render(
      <HTML5DropTarget onFilesReceived={onFilesReceived} />,
    );

    const target = screen.container.querySelector(
      '[data-testid="drop-target"]',
    )!;

    // Create a DataTransfer with a real File
    const dataTransfer = new DataTransfer();
    const testFile = new File(["hello world"], "test.txt", {
      type: "text/plain",
    });
    dataTransfer.items.add(testFile);

    // Simulate the full drag sequence
    target.dispatchEvent(
      new DragEvent("dragover", {
        bubbles: true,
        cancelable: true,
        dataTransfer,
      }),
    );
    target.dispatchEvent(
      new DragEvent("drop", { bubbles: true, cancelable: true, dataTransfer }),
    );

    expect(onFilesReceived).toHaveBeenCalledTimes(1);
    const files = onFilesReceived.mock.calls[0][0];
    expect(files).toHaveLength(1);
    expect(files[0].name).toBe("test.txt");
    expect(files[0].size).toBe(11); // "hello world".length
    expect(files[0].type).toBe("text/plain");
  });

  it("receives multiple files", async () => {
    const onFilesReceived = vi.fn();
    const screen = await render(
      <HTML5DropTarget onFilesReceived={onFilesReceived} />,
    );

    const target = screen.container.querySelector(
      '[data-testid="drop-target"]',
    )!;

    const dataTransfer = new DataTransfer();
    dataTransfer.items.add(
      new File(["aaa"], "doc.pdf", { type: "application/pdf" }),
    );
    dataTransfer.items.add(
      new File(["bbb"], "photo.png", { type: "image/png" }),
    );

    target.dispatchEvent(
      new DragEvent("drop", { bubbles: true, cancelable: true, dataTransfer }),
    );

    expect(onFilesReceived).toHaveBeenCalledTimes(1);
    const files = onFilesReceived.mock.calls[0][0];
    expect(files).toHaveLength(2);
    expect(files[0].name).toBe("doc.pdf");
    expect(files[1].name).toBe("photo.png");
  });

  it("dataTransfer.files is empty when only task MIME is set (no File objects)", async () => {
    const onFilesReceived = vi.fn();
    const screen = await render(
      <HTML5DropTarget onFilesReceived={onFilesReceived} />,
    );

    const target = screen.container.querySelector(
      '[data-testid="drop-target"]',
    )!;

    // Task card drag — setData with MIME, no File objects
    const dataTransfer = new DataTransfer();
    dataTransfer.setData(
      "application/x-swissarmyhammer-task",
      '{"id":"task-1"}',
    );

    target.dispatchEvent(
      new DragEvent("drop", { bubbles: true, cancelable: true, dataTransfer }),
    );

    expect(onFilesReceived).toHaveBeenCalledTimes(1);
    const files = onFilesReceived.mock.calls[0][0];
    expect(files).toHaveLength(0);
  });

  it("can read file content via FileReader after drop", async () => {
    // Test that File objects from DataTransfer are readable
    const dataTransfer = new DataTransfer();
    const content = "file content here";
    const file = new File([content], "readme.md", { type: "text/markdown" });
    dataTransfer.items.add(file);

    // Read directly from the DataTransfer (simulating what drop handler would do)
    const droppedFile = dataTransfer.files[0];
    expect(droppedFile).toBeTruthy();
    expect(droppedFile.name).toBe("readme.md");

    const text = await droppedFile.text();
    expect(text).toBe("file content here");
  });

  it("HTML5 drop target and task card DropZone can coexist", async () => {
    // This proves the key insight: we can distinguish file drops from
    // task drops using dataTransfer.types, then handle each differently
    const fileHandler = vi.fn();
    const taskHandler = vi.fn();

    function DualDropTarget() {
      const handleDrop = useCallback((e: React.DragEvent) => {
        e.preventDefault();
        if (e.dataTransfer.types.includes("Files")) {
          const files = Array.from(e.dataTransfer.files);
          fileHandler(files.map((f) => f.name));
        } else if (
          e.dataTransfer.types.includes("application/x-swissarmyhammer-task")
        ) {
          taskHandler(
            e.dataTransfer.getData("application/x-swissarmyhammer-task"),
          );
        }
      }, []);

      return (
        <div
          data-testid="dual-target"
          onDragOver={(e) => e.preventDefault()}
          onDrop={handleDrop}
          style={{ width: 300, height: 200 }}
        >
          Drop anything
        </div>
      );
    }

    const screen = await render(<DualDropTarget />);
    const target = screen.container.querySelector(
      '[data-testid="dual-target"]',
    )!;

    // Drop a file
    const fileDT = new DataTransfer();
    fileDT.items.add(new File(["x"], "photo.jpg", { type: "image/jpeg" }));
    target.dispatchEvent(
      new DragEvent("drop", {
        bubbles: true,
        cancelable: true,
        dataTransfer: fileDT,
      }),
    );

    expect(fileHandler).toHaveBeenCalledWith(["photo.jpg"]);
    expect(taskHandler).not.toHaveBeenCalled();

    // Drop a task card
    const taskDT = new DataTransfer();
    taskDT.setData("application/x-swissarmyhammer-task", '{"id":"task-42"}');
    target.dispatchEvent(
      new DragEvent("drop", {
        bubbles: true,
        cancelable: true,
        dataTransfer: taskDT,
      }),
    );

    expect(taskHandler).toHaveBeenCalledWith('{"id":"task-42"}');
    expect(fileHandler).toHaveBeenCalledTimes(1); // still just the one file call
  });
});
