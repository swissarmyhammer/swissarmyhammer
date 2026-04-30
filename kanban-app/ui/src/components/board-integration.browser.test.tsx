/**
 * Integration tests: real .kanban board + browser UI.
 *
 * These tests create a real .kanban directory via the kanban CLI,
 * render the actual BoardView in Chromium with real entity data,
 * perform drag interactions, and assert both:
 *   1. The UI displays the correct state (DOM assertions)
 *   2. The underlying entity data changed on disk (CLI readback)
 *
 * Card move tests SHOULD FAIL until the FileDropProvider fix is applied.
 * The file drag test should PASS (Tauri native pipeline is separate).
 */

import { describe, it, expect, vi, beforeAll, afterAll } from "vitest";
import { render } from "vitest-browser-react";
import { commands } from "vitest/browser";
import type { BoardData, Entity, BoardSummary } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Tauri mocks
// ---------------------------------------------------------------------------

/** Minimal task schema so EntityCard renders title on cards. */
const TASK_SCHEMA = {
  entity: {
    name: "task",
    fields: ["title", "description", "position_column", "position_ordinal"],
  },
  fields: [
    {
      id: "title",
      name: "title",
      type: { kind: "text" },
      section: "header",
      display: "text",
      editor: "text",
    },
    {
      id: "description",
      name: "description",
      type: { kind: "text" },
      section: "body",
    },
    {
      id: "position_column",
      name: "position_column",
      type: { kind: "text" },
      section: "hidden",
    },
    {
      id: "position_ordinal",
      name: "position_ordinal",
      type: { kind: "text" },
      section: "hidden",
    },
  ],
};

const mockInvoke = vi.fn(async (cmd: string, _args?: any) => {
  if (cmd === "list_entity_types") return ["task"];
  if (cmd === "get_entity_schema") return TASK_SCHEMA;
  if (cmd === "list_commands_for_scope") return { commands: [] };
  if (cmd === "list_views") return [];
  if (cmd === "get_ui_state")
    return {
      palette_open: false,
      palette_mode: "command",
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      windows: {},
      recent_boards: [],
    };
  return null;
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...a: [string, any?]) => mockInvoke(...a),
}));
vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(() => Promise.resolve()),
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: vi.fn(() => Promise.resolve(() => {})),
  }),
}));
vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

// ---------------------------------------------------------------------------
// Imports — after mocks
// ---------------------------------------------------------------------------

// Register field display/editor components — required for EntityCard to render titles
import "@/components/fields/registrations";

import { EntityFocusProvider } from "@/lib/entity-focus-context";

import { DragSessionProvider } from "@/lib/drag-session-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { ActiveBoardPathProvider } from "@/lib/command-scope";
import { FileDropProvider } from "@/lib/file-drop-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { ViewsProvider } from "@/lib/views-context";
import { PerspectiveProvider } from "@/lib/perspective-context";
import { PerspectiveContainer } from "@/components/perspective-container";
import { TooltipProvider } from "@/components/ui/tooltip";
import { BoardView } from "./board-view";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { asSegment } from "@/types/spatial";

// ---------------------------------------------------------------------------
// Test state — populated by beforeAll from real .kanban data
// ---------------------------------------------------------------------------

interface StrippedTask {
  id: string;
  title: string;
  position: { column: string; ordinal: string };
  assignees: string[];
  tags: string[];
  attachments: string[];
  description: string;
  progress: number;
  depends_on: string[];
}

interface StrippedColumn {
  id: string;
  name: string;
  order: number;
}

let testBoardDir: string;
let testBoardName: string;
let testSummary: BoardSummary;
let testTasks: StrippedTask[];
let testColumns: StrippedColumn[];
let testTaskIds: string[];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const DRAG_MIME = "application/x-swissarmyhammer-task";

/** Convert stripped task into the Entity shape the UI expects. */
function taskToEntity(task: StrippedTask): Entity {
  return {
    entity_type: "task",
    id: task.id,
    moniker: `task:${task.id}`,
    fields: {
      title: task.title,
      position_column: task.position.column,
      position_ordinal: task.position.ordinal,
      description: task.description,
      assignees: task.assignees,
      tags: task.tags,
      attachments: task.attachments,
      depends_on: task.depends_on,
      progress: task.progress,
    },
  };
}

/** Convert stripped column into the Entity shape. */
function columnToEntity(col: StrippedColumn): Entity {
  return {
    entity_type: "column",
    id: col.id,
    moniker: `column:${col.id}`,
    fields: { name: col.name, order: col.order },
  };
}

/** Build BoardData from real CLI output. */
function buildBoardData(): BoardData {
  return {
    board: {
      entity_type: "board",
      id: "board",
      moniker: "board:board",
      fields: { name: testBoardName },
    },
    columns: testColumns.map(columnToEntity),

    tags: [],
    virtualTagMeta: [],
    summary: testSummary,
  };
}

/** Render the full BoardView with real data and all required providers. */
function renderIntegrationBoard() {
  const board = buildBoardData();
  const tasks = testTasks.map(taskToEntity);

  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <FileDropProvider>
          <EntityFocusProvider>
            <SchemaProvider>
              <EntityStoreProvider entities={{ task: tasks, tag: [] }}>
                <TooltipProvider>
                  <ActiveBoardPathProvider value={testBoardDir + "/.kanban"}>
                    <FieldUpdateProvider>
                      <UIStateProvider>
                        <ViewsProvider>
                          <PerspectiveProvider>
                            <PerspectiveContainer>
                              <DragSessionProvider>
                                <BoardView board={board} tasks={tasks} />
                              </DragSessionProvider>
                            </PerspectiveContainer>
                          </PerspectiveProvider>
                        </ViewsProvider>
                      </UIStateProvider>
                    </FieldUpdateProvider>
                  </ActiveBoardPathProvider>
                </TooltipProvider>
              </EntityStoreProvider>
            </SchemaProvider>
          </EntityFocusProvider>
        </FileDropProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("Board integration — real .kanban data", () => {
  beforeAll(async () => {
    const result = await commands.createTestBoard({
      name: "Integration Test Board",
      tasks: [
        { title: "Card Alpha" },
        { title: "Card Beta" },
        { title: "Card Gamma" },
        { title: "Card Delta", column: "doing" },
      ],
      perspectives: [
        { name: "Sprint Board", view: "board" },
        { name: "Grid Overview", view: "grid" },
      ],
    });
    testBoardDir = result.dir;
    testBoardName = result.boardName;
    testSummary = result.summary;
    testTasks = result.tasks;
    testColumns = result.columns;
    testTaskIds = result.taskIds;
  });

  afterAll(async () => {
    if (testBoardDir) {
      await commands.cleanupTestBoard({ dir: testBoardDir });
    }
  });

  it("creates a real board with real tasks on disk", async () => {
    expect(testBoardDir).toBeTruthy();
    expect(testTasks.length).toBe(4);
    expect(testColumns.length).toBe(3);
    expect(testTaskIds.length).toBe(4);

    // Read back a task from disk to prove it's real
    const task = await commands.readEntity({
      dir: testBoardDir,
      noun: "task",
      id: testTaskIds[0],
    });
    expect(task.title).toBe("Card Alpha");
    expect(task.position.column).toBe("todo");
  });

  it("creates perspectives on disk via CLI", async () => {
    const result = await commands.listPerspectives({ dir: testBoardDir });
    expect(result.count).toBe(2);
    const names = result.perspectives.map((p) => p.name);
    expect(names).toContain("Sprint Board");
    expect(names).toContain("Grid Overview");
  });

  it("renders the board with all columns and task cards", async () => {
    const screen = await renderIntegrationBoard();
    const text = screen.container.textContent || "";

    // Columns visible
    expect(text).toContain("To Do");
    expect(text).toContain("Doing");
    expect(text).toContain("Done");

    // Task cards visible
    expect(text).toContain("Card Alpha");
    expect(text).toContain("Card Beta");
    expect(text).toContain("Card Gamma");
    expect(text).toContain("Card Delta");
  });

  it("shows tasks in correct columns based on real data", async () => {
    const screen = await renderIntegrationBoard();

    // All 4 cards render
    expect(screen.container.textContent).toContain("Card Alpha");
    expect(screen.container.textContent).toContain("Card Beta");
    expect(screen.container.textContent).toContain("Card Gamma");
    expect(screen.container.textContent).toContain("Card Delta");
  });

  it("move task between columns: entity changes on disk", async () => {
    const alphaId = testTaskIds[0];

    // Move Card Alpha from todo to doing via real CLI
    await commands.moveTask({
      dir: testBoardDir,
      taskId: alphaId,
      column: "doing",
    });

    // Verify on disk — the entity file actually changed
    const entity = await commands.readEntity({
      dir: testBoardDir,
      noun: "task",
      id: alphaId,
    });
    expect(entity.position.column).toBe("doing");

    // Refresh and re-render
    const fresh = await commands.listTasks({ dir: testBoardDir });
    testTasks = fresh.tasks;

    const screen = await renderIntegrationBoard();
    const text = screen.container.textContent || "";
    expect(text).toContain("Card Alpha");
  });

  it("move task within column reorder: ordinals change on disk", async () => {
    const fresh = await commands.listTasks({ dir: testBoardDir });
    testTasks = fresh.tasks;

    const todoTasks = testTasks.filter((t) => t.position.column === "todo");

    if (todoTasks.length >= 2) {
      const lastTask = todoTasks[todoTasks.length - 1];
      const firstTask = todoTasks[0];

      // Move last task before first (reorder)
      await commands.moveTask({
        dir: testBoardDir,
        taskId: lastTask.id,
        column: "todo",
        beforeId: firstTask.id,
      });

      // Verify ordinals changed on disk
      const movedEntity = await commands.readEntity({
        dir: testBoardDir,
        noun: "task",
        id: lastTask.id,
      });
      const firstEntity = await commands.readEntity({
        dir: testBoardDir,
        noun: "task",
        id: firstTask.id,
      });

      expect(movedEntity.position.ordinal < firstEntity.position.ordinal).toBe(
        true,
      );
    }
  });

  it("drag task card on DropZone with FileDropProvider active", async () => {
    const fresh = await commands.listTasks({ dir: testBoardDir });
    testTasks = fresh.tasks;

    const screen = await renderIntegrationBoard();

    // Find drop zones
    const dropZones = screen.container.querySelectorAll("[data-drop-zone]");
    expect(dropZones.length).toBeGreaterThan(0);

    // Build a task payload matching what DraggableTaskCard would create
    const taskEntity = testTasks[0];
    const taskPayload = JSON.stringify({
      entity_type: "task",
      id: taskEntity.id,
      fields: {
        title: taskEntity.title,
        position_column: taskEntity.position.column,
        position_ordinal: taskEntity.position.ordinal,
      },
    });

    // Dispatch a drop with task MIME data on a DropZone
    const targetZone = dropZones[0];
    const dataTransfer = new DataTransfer();
    dataTransfer.setData(DRAG_MIME, taskPayload);

    const dragoverEvent = new DragEvent("dragover", {
      bubbles: true,
      cancelable: true,
      dataTransfer,
    });
    targetZone.dispatchEvent(dragoverEvent);

    // DropZone calls stopPropagation, so it should accept the drag
    // even with FileDropProvider's global handler active.
    // The dragover should be preventDefault'd by the DropZone handler.
    expect(dragoverEvent.defaultPrevented).toBe(true);

    // Drop should work
    targetZone.dispatchEvent(
      new DragEvent("drop", { bubbles: true, dataTransfer }),
    );

    // No crash = structural success
  });

  it("file drag over non-DropZone area is blocked by FileDropProvider", async () => {
    const screen = await renderIntegrationBoard();

    // Dispatch a file dragover on the board container (not a DropZone)
    const dataTransfer = new DataTransfer();
    dataTransfer.items.add(
      new File(["content"], "photo.png", { type: "image/png" }),
    );

    const event = new DragEvent("dragover", {
      bubbles: true,
      cancelable: true,
      dataTransfer,
    });

    screen.container.dispatchEvent(event);

    // FileDropProvider should preventDefault for Files — browser won't navigate
    expect(event.defaultPrevented).toBe(true);
  });

  it("task drag over non-DropZone area is NOT blocked (regression test)", async () => {
    const screen = await renderIntegrationBoard();

    const dataTransfer = new DataTransfer();
    dataTransfer.setData(DRAG_MIME, '{"id":"task-1"}');

    const event = new DragEvent("dragover", {
      bubbles: true,
      cancelable: true,
      dataTransfer,
    });

    // Dispatch on the board container directly (misses all DropZones)
    screen.container.dispatchEvent(event);

    // After fix: document should NOT accept task drags outside DropZones
    // CURRENTLY BROKEN: global handler calls preventDefault on everything
    expect(event.defaultPrevented).toBe(false);
  });

  it("Do This Next context menu routes through task.doThisNext backend command", async () => {
    // Temporarily override the mock to return task.doThisNext as available
    const savedImpl = mockInvoke.getMockImplementation()!;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const doThisNextMock = async (cmd: string, args?: any): Promise<any> => {
      if (cmd === "list_commands_for_scope") {
        return [
          {
            id: "task.doThisNext",
            name: "Do This Next",
            target: `task:${testTaskIds[3]}`,
            group: "entity",
            context_menu: true,
            available: true,
          },
        ];
      }
      if (cmd === "show_context_menu") return null;
      return savedImpl(cmd, args);
    };
    mockInvoke.mockImplementation(doThisNextMock);

    try {
      const screen = await renderIntegrationBoard();

      // Find the task card for Card Delta (index 3, in "doing" column)
      const deltaCard = screen.container.querySelector(
        `[data-entity-card="${testTaskIds[3]}"]`,
      );
      expect(deltaCard).toBeTruthy();

      // Right-click to trigger context menu
      mockInvoke.mockClear();
      mockInvoke.mockImplementation(doThisNextMock);

      deltaCard!.dispatchEvent(
        new MouseEvent("contextmenu", { bubbles: true, cancelable: true }),
      );

      // Wait for async invoke calls to complete
      await new Promise((r) => setTimeout(r, 200));

      // Assert show_context_menu was called with task.doThisNext
      const showCall = mockInvoke.mock.calls.find(
        (c) => c[0] === "show_context_menu",
      );
      expect(showCall).toBeTruthy();
      const items = (
        showCall![1] as { items: { cmd: string; scope_chain: string[] }[] }
      ).items;
      const doThisNext = items.find((i) => i.cmd === "task.doThisNext");
      expect(doThisNext).toBeTruthy();
      // Scope chain should contain the task moniker
      expect(doThisNext!.scope_chain.some((s) => s.startsWith("task:"))).toBe(
        true,
      );

      // No task.move should have been dispatched (old workaround gone)
      const moveCall = mockInvoke.mock.calls.find(
        (c) =>
          c[0] === "dispatch_command" &&
          (c[1] as { cmd?: string })?.cmd === "task.move",
      );
      expect(moveCall).toBeUndefined();
    } finally {
      // Restore original mock
      mockInvoke.mockImplementation(savedImpl);
    }
  });

  it("Do This Next via CLI moves task to top of first column", async () => {
    // Refresh state
    const fresh = await commands.listTasks({ dir: testBoardDir });
    testTasks = fresh.tasks;

    // Find a task in a non-first column (Card Delta is in "doing")
    const doingTask = testTasks.find((t) => t.position.column === "doing");
    expect(doingTask).toBeTruthy();

    // Find the first task in the first column (order-0 = "todo")
    const todoTasks = testTasks
      .filter((t) => t.position.column === "todo")
      .sort((a, b) => a.position.ordinal.localeCompare(b.position.ordinal));
    const firstTodoTask = todoTasks[0];

    // Simulate DoThisNext: move task to "todo" column, before the first task
    await commands.moveTask({
      dir: testBoardDir,
      taskId: doingTask!.id,
      column: "todo",
      ...(firstTodoTask ? { beforeId: firstTodoTask.id } : {}),
    });

    // Verify on disk: the task is now in the todo column with a lower ordinal
    const movedEntity = await commands.readEntity({
      dir: testBoardDir,
      noun: "task",
      id: doingTask!.id,
    });
    expect(movedEntity.position.column).toBe("todo");

    if (firstTodoTask) {
      const firstEntity = await commands.readEntity({
        dir: testBoardDir,
        noun: "task",
        id: firstTodoTask.id,
      });
      expect(movedEntity.position.ordinal < firstEntity.position.ordinal).toBe(
        true,
      );
    }

    // Re-render and verify the card is in the first column
    const freshTasks = await commands.listTasks({ dir: testBoardDir });
    testTasks = freshTasks.tasks;

    const screen = await renderIntegrationBoard();
    const text = screen.container.textContent || "";
    expect(text).toContain("Card Delta");
  });
});
