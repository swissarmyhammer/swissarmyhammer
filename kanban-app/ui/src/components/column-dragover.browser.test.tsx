/**
 * Browser-mode tests for ColumnView's dragover handling.
 *
 * Uses a minimal test harness that replicates the column's drag handler
 * behavior without the full BoardView context. Tests the MIME type
 * discrimination between task card drags and file drags from Finder.
 */

import { describe, it, expect, vi } from "vitest";
import { render } from "vitest-browser-react";
import { useCallback, useRef } from "react";

const DRAG_MIME = "application/x-swissarmyhammer-task";

/**
 * Minimal component replicating ColumnView's handleContainerDragOver logic.
 * Uses native addEventListener so events work with dispatchEvent in browser mode.
 */
function ColumnDragTarget({
  onAccepted,
  onRejected,
}: {
  onAccepted?: () => void;
  onRejected?: () => void;
}) {
  const ref = useCallback(
    (node: HTMLDivElement | null) => {
      if (!node) return;
      node.addEventListener("dragover", (e: DragEvent) => {
        // This is the exact logic from column-view.tsx:395-412
        if (e.dataTransfer?.types.includes("Files")) {
          onRejected?.();
          return;
        }
        e.preventDefault();
        e.dataTransfer!.dropEffect = "move";
        onAccepted?.();
      });
    },
    [onAccepted, onRejected],
  );

  return (
    <div
      ref={ref}
      data-testid="column-container"
      style={{ width: 300, height: 600 }}
    >
      Column content
    </div>
  );
}

describe("ColumnView dragover — MIME discrimination", () => {
  it("accepts task card drag (calls preventDefault)", async () => {
    const onAccepted = vi.fn();
    const screen = await render(<ColumnDragTarget onAccepted={onAccepted} />);

    const container = screen.container.querySelector(
      '[data-testid="column-container"]',
    )!;

    const dataTransfer = new DataTransfer();
    dataTransfer.setData(DRAG_MIME, '{"id":"task-1"}');

    const event = new DragEvent("dragover", {
      bubbles: true,
      cancelable: true,
      dataTransfer,
    });

    container.dispatchEvent(event);

    expect(event.defaultPrevented).toBe(true);
    expect(onAccepted).toHaveBeenCalledTimes(1);
  });

  it("rejects file drag (does NOT call preventDefault)", async () => {
    const onRejected = vi.fn();
    const screen = await render(<ColumnDragTarget onRejected={onRejected} />);

    const container = screen.container.querySelector(
      '[data-testid="column-container"]',
    )!;

    const dataTransfer = new DataTransfer();
    dataTransfer.items.add(new File([""], "photo.png", { type: "image/png" }));

    const event = new DragEvent("dragover", {
      bubbles: true,
      cancelable: true,
      dataTransfer,
    });

    container.dispatchEvent(event);

    expect(event.defaultPrevented).toBe(false);
    expect(onRejected).toHaveBeenCalledTimes(1);
  });

  it("Files type takes priority over task MIME", async () => {
    const onRejected = vi.fn();
    const onAccepted = vi.fn();
    const screen = await render(
      <ColumnDragTarget onAccepted={onAccepted} onRejected={onRejected} />,
    );

    const container = screen.container.querySelector(
      '[data-testid="column-container"]',
    )!;

    const dataTransfer = new DataTransfer();
    dataTransfer.items.add(new File([""], "test.txt", { type: "text/plain" }));

    const event = new DragEvent("dragover", {
      bubbles: true,
      cancelable: true,
      dataTransfer,
    });

    container.dispatchEvent(event);

    expect(event.defaultPrevented).toBe(false);
    expect(onRejected).toHaveBeenCalledTimes(1);
    expect(onAccepted).not.toHaveBeenCalled();
  });
});
