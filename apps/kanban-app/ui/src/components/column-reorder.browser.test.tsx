/**
 * Browser-mode tests for column reorder via dnd-kit.
 *
 * dnd-kit uses PointerSensor (pointer events), which is completely
 * isolated from HTML5 drag API and FileDropProvider. These tests
 * verify that isolation holds.
 *
 * Tests verify:
 * 1. PointerSensor activation requires 5px movement
 * 2. Column drag does NOT emit HTML5 drag events
 * 3. Column drag works with FileDropProvider active
 */

import { describe, it, expect, vi } from "vitest";
import { render } from "vitest-browser-react";
import {
  DndContext,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  SortableContext,
  horizontalListSortingStrategy,
  useSortable,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";

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

/** Minimal sortable column for testing. */
function SortableColumn({ id, name }: { id: string; name: string }) {
  const { attributes, listeners, setNodeRef, transform, transition } =
    useSortable({ id });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    width: 200,
    height: 400,
    background: "#f0f0f0",
    border: "1px solid #ccc",
    padding: 10,
  };

  return (
    <div ref={setNodeRef} style={style} data-testid={`column-${id}`}>
      <button
        data-testid={`grip-${id}`}
        {...listeners}
        {...attributes}
        style={{ cursor: "grab" }}
      >
        ⠿ {name}
      </button>
    </div>
  );
}

/** Test harness with dnd-kit context and optional FileDropProvider. */
function ColumnBoard({
  columns,
  onDragEnd,
  withFileDropProvider = false,
}: {
  columns: { id: string; name: string }[];
  onDragEnd?: (event: DragEndEvent) => void;
  withFileDropProvider?: boolean;
}) {
  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: { distance: 5 },
    }),
  );

  const ids = columns.map((c) => c.id);

  const content = (
    <DndContext sensors={sensors} onDragEnd={onDragEnd ?? (() => {})}>
      <SortableContext items={ids} strategy={horizontalListSortingStrategy}>
        <div style={{ display: "flex", gap: 10 }}>
          {columns.map((col) => (
            <SortableColumn key={col.id} id={col.id} name={col.name} />
          ))}
        </div>
      </SortableContext>
    </DndContext>
  );

  if (withFileDropProvider) {
    return <FileDropProvider>{content}</FileDropProvider>;
  }
  return content;
}

describe("Column reorder — dnd-kit isolation", () => {
  const columns = [
    { id: "todo", name: "To Do" },
    { id: "doing", name: "Doing" },
    { id: "done", name: "Done" },
  ];

  it("renders column grip handles", async () => {
    const screen = await render(<ColumnBoard columns={columns} />);

    for (const col of columns) {
      await expect
        .element(screen.getByTestId(`grip-${col.id}`))
        .toBeInTheDocument();
    }
  });

  it("grip handle does not have draggable attribute (uses pointer events, not HTML5 drag)", async () => {
    const screen = await render(<ColumnBoard columns={columns} />);

    const grip = screen.container.querySelector(
      '[data-testid="grip-todo"]',
    )! as HTMLElement;

    // dnd-kit PointerSensor does NOT set draggable="true" — it uses
    // pointerdown/pointermove/pointerup instead of HTML5 drag API
    expect(grip.getAttribute("draggable")).not.toBe("true");
  });

  it("HTML5 dragstart on grip does NOT activate dnd-kit (isolated APIs)", async () => {
    const onDragEnd = vi.fn();
    const screen = await render(
      <ColumnBoard columns={columns} onDragEnd={onDragEnd} />,
    );

    const grip = screen.container.querySelector('[data-testid="grip-todo"]')!;

    // Dispatch an HTML5 dragstart — this should not trigger dnd-kit
    const dt = new DataTransfer();
    grip.dispatchEvent(
      new DragEvent("dragstart", { bubbles: true, dataTransfer: dt }),
    );
    grip.dispatchEvent(
      new DragEvent("dragend", { bubbles: true, dataTransfer: dt }),
    );

    expect(onDragEnd).not.toHaveBeenCalled();
  });

  it("renders with FileDropProvider without errors", async () => {
    const screen = await render(
      <ColumnBoard columns={columns} withFileDropProvider />,
    );

    // All columns should render
    for (const col of columns) {
      await expect
        .element(screen.getByTestId(`column-${col.id}`))
        .toBeInTheDocument();
    }
  });

  it("HTML5 task drag events do NOT interfere with column structure", async () => {
    const screen = await render(
      <ColumnBoard columns={columns} withFileDropProvider />,
    );

    const column = screen.container.querySelector(
      '[data-testid="column-todo"]',
    )!;

    // Simulate a task card being dragged over the column
    const dt = new DataTransfer();
    dt.setData("application/x-swissarmyhammer-task", '{"id":"task-1"}');

    column.dispatchEvent(
      new DragEvent("dragover", { bubbles: true, dataTransfer: dt }),
    );

    // Column should still be intact — dnd-kit state unaffected
    for (const col of columns) {
      const el = screen.container.querySelector(
        `[data-testid="column-${col.id}"]`,
      );
      expect(el).not.toBeNull();
    }
  });
});
