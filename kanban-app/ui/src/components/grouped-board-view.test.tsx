import { describe, it, expect, vi } from "vitest";
import { render } from "@testing-library/react";
import { GroupedBoardView } from "./grouped-board-view";
import type { BoardData, Entity } from "@/types/kanban";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
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

// Track what useActivePerspective returns — overridden per test.
let mockGroupField: string | undefined;
let mockFieldDefs: import("@/types/kanban").FieldDef[] = [];

vi.mock("@/components/perspective-container", () => ({
  useActivePerspective: () => ({
    activePerspective: null,
    applySort: (entities: unknown[]) => entities,
    groupField: mockGroupField,
  }),
}));

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: (type: string) =>
      type === "task" ? { fields: mockFieldDefs } : undefined,
    getFieldDef: () => undefined,
    getEntityCommands: () => [],
    loading: false,
    mentionableTypes: [],
  }),
  useSchemaOptional: () => ({
    getSchema: () => undefined,
    getFieldDef: () => undefined,
    getEntityCommands: () => [],
  }),
  SchemaProvider: ({ children }: { children: React.ReactNode }) => children,
}));

// Mock BoardView to inspect what it receives
const BoardViewMock = vi.fn(
  ({ tasks }: { board: BoardData; tasks: Entity[] }) => (
    <div data-testid="board-view">
      {tasks.map((t) => (
        <div key={t.id} data-testid={`task-${t.id}`}>
          {String(t.fields.title)}
        </div>
      ))}
    </div>
  ),
);

vi.mock("@/components/board-view", () => ({
  BoardView: (props: { board: BoardData; tasks: Entity[] }) =>
    BoardViewMock(props),
}));

function makeColumn(id: string, name: string, order: number): Entity {
  return {
    id,
    entity_type: "column",
    moniker: `column:${id}`,
    fields: { name, order },
  };
}

function makeTask(id: string, column: string, groupValue?: string): Entity {
  const fields: Record<string, unknown> = {
    title: `Task ${id}`,
    position_column: column,
    position_ordinal: "a0",
  };
  if (groupValue !== undefined) fields.project = groupValue;
  return { id, entity_type: "task", moniker: `task:${id}`, fields };
}

const board: BoardData = {
  board: {
    id: "b1",
    entity_type: "board",
    moniker: "board:b1",
    fields: { name: "Board" },
  },
  columns: [makeColumn("todo", "Todo", 0), makeColumn("doing", "Doing", 1)],
  tags: [],
  summary: {
    total_tasks: 0,
    total_actors: 0,
    ready_tasks: 0,
    blocked_tasks: 0,
    done_tasks: 0,
    percent_complete: 0,
  },
};

describe("GroupedBoardView", () => {
  it("renders BoardView directly when no groupField", () => {
    mockGroupField = undefined;
    BoardViewMock.mockClear();

    const tasks = [makeTask("t1", "todo"), makeTask("t2", "doing")];
    const { getAllByTestId } = render(
      <GroupedBoardView board={board} tasks={tasks} />,
    );

    // Should render a single BoardView with all tasks
    expect(BoardViewMock).toHaveBeenCalledTimes(1);
    expect(BoardViewMock).toHaveBeenCalledWith(
      expect.objectContaining({ board, tasks }),
    );
    expect(getAllByTestId("board-view")).toHaveLength(1);
  });

  it("renders group sections when groupField is active", () => {
    mockGroupField = "project";
    mockFieldDefs = [
      {
        id: "project",
        name: "project",
        type: { kind: "string" },
      } as import("@/types/kanban").FieldDef,
    ];
    BoardViewMock.mockClear();

    const tasks = [
      makeTask("t1", "todo", "alpha"),
      makeTask("t2", "doing", "beta"),
      makeTask("t3", "todo", "alpha"),
    ];

    const { getByText, getAllByTestId } = render(
      <GroupedBoardView board={board} tasks={tasks} />,
    );

    // Should have group headers
    expect(getByText("alpha")).toBeTruthy();
    expect(getByText("beta")).toBeTruthy();
    // Each group section should render a BoardView
    expect(getAllByTestId("board-view").length).toBeGreaterThanOrEqual(2);
  });

  it("shows correct task count per section", () => {
    mockGroupField = "project";
    mockFieldDefs = [
      {
        id: "project",
        name: "project",
        type: { kind: "string" },
      } as import("@/types/kanban").FieldDef,
    ];
    BoardViewMock.mockClear();

    const tasks = [
      makeTask("t1", "todo", "alpha"),
      makeTask("t2", "doing", "alpha"),
      makeTask("t3", "todo", "beta"),
    ];

    const { getByText } = render(
      <GroupedBoardView board={board} tasks={tasks} />,
    );

    // alpha has 2 tasks, beta has 1
    expect(getByText("2")).toBeTruthy();
    expect(getByText("1")).toBeTruthy();
  });

  it("puts ungrouped tasks in an (ungrouped) section at the bottom", () => {
    mockGroupField = "project";
    mockFieldDefs = [
      {
        id: "project",
        name: "project",
        type: { kind: "string" },
      } as import("@/types/kanban").FieldDef,
    ];
    BoardViewMock.mockClear();

    const tasks = [
      makeTask("t1", "todo", "alpha"),
      makeTask("t2", "doing"), // no project field
    ];

    const { getByText } = render(
      <GroupedBoardView board={board} tasks={tasks} />,
    );

    expect(getByText("(ungrouped)")).toBeTruthy();
  });

  it("passes correct groupValue to each BoardView section", () => {
    mockGroupField = "project";
    mockFieldDefs = [
      {
        id: "project",
        name: "project",
        type: { kind: "string" },
      } as import("@/types/kanban").FieldDef,
    ];
    BoardViewMock.mockClear();

    const tasks = [
      makeTask("t1", "todo", "alpha"),
      makeTask("t2", "doing", "beta"),
    ];

    render(<GroupedBoardView board={board} tasks={tasks} />);

    // BoardView should be called once per group, each with only its group's tasks
    expect(BoardViewMock).toHaveBeenCalledTimes(2);
    const firstCallTasks = BoardViewMock.mock.calls[0][0].tasks;
    const secondCallTasks = BoardViewMock.mock.calls[1][0].tasks;
    // alpha sorts before beta
    expect(firstCallTasks).toHaveLength(1);
    expect(firstCallTasks[0].id).toBe("t1");
    expect(secondCallTasks).toHaveLength(1);
    expect(secondCallTasks[0].id).toBe("t2");
  });

  it("groups are sorted alphabetically with (ungrouped) last", () => {
    mockGroupField = "project";
    mockFieldDefs = [
      {
        id: "project",
        name: "project",
        type: { kind: "string" },
      } as import("@/types/kanban").FieldDef,
    ];
    BoardViewMock.mockClear();

    const tasks = [
      makeTask("t1", "todo"), // ungrouped
      makeTask("t2", "doing", "zebra"),
      makeTask("t3", "todo", "alpha"),
    ];

    const { container } = render(
      <GroupedBoardView board={board} tasks={tasks} />,
    );

    // Extract group header text in DOM order
    const buttons = container.querySelectorAll("button");
    const labels = Array.from(buttons).map((b) => b.textContent);

    // alpha should come before zebra, (ungrouped) last
    const alphaIdx = labels.findIndex((l) => l?.includes("alpha"));
    const zebraIdx = labels.findIndex((l) => l?.includes("zebra"));
    const ungroupedIdx = labels.findIndex((l) => l?.includes("(ungrouped)"));

    expect(alphaIdx).toBeLessThan(zebraIdx);
    expect(zebraIdx).toBeLessThan(ungroupedIdx);
  });

  it("each group section only contains tasks for that group", () => {
    mockGroupField = "project";
    mockFieldDefs = [
      {
        id: "project",
        name: "project",
        type: { kind: "string" },
      } as import("@/types/kanban").FieldDef,
    ];
    BoardViewMock.mockClear();

    const tasks = [
      makeTask("t1", "todo", "alpha"),
      makeTask("t2", "doing", "alpha"),
      makeTask("t3", "todo", "beta"),
      makeTask("t4", "doing"), // ungrouped
    ];

    render(<GroupedBoardView board={board} tasks={tasks} />);

    // 3 groups: alpha (2 tasks), beta (1 task), ungrouped (1 task)
    expect(BoardViewMock).toHaveBeenCalledTimes(3);

    const allCalls = BoardViewMock.mock.calls.map(
      (c: [{ tasks: Entity[] }]) => c[0].tasks,
    );
    const alphaTasks = allCalls.find(
      (t: Entity[]) => t.length === 2 && t[0].fields.project === "alpha",
    );
    const betaTasks = allCalls.find(
      (t: Entity[]) => t.length === 1 && t[0].fields.project === "beta",
    );
    const ungroupedTasks = allCalls.find(
      (t: Entity[]) => t.length === 1 && t[0].fields.project === undefined,
    );

    expect(alphaTasks).toBeDefined();
    expect(betaTasks).toBeDefined();
    expect(ungroupedTasks).toBeDefined();
  });

  it("removing groupField reverts to flat board view", () => {
    mockGroupField = "project";
    mockFieldDefs = [
      {
        id: "project",
        name: "project",
        type: { kind: "string" },
      } as import("@/types/kanban").FieldDef,
    ];
    BoardViewMock.mockClear();

    const tasks = [
      makeTask("t1", "todo", "alpha"),
      makeTask("t2", "doing", "beta"),
    ];

    const { rerender, queryByText, getAllByTestId } = render(
      <GroupedBoardView board={board} tasks={tasks} />,
    );

    // Grouped: sections visible
    expect(queryByText("alpha")).toBeTruthy();
    expect(queryByText("beta")).toBeTruthy();

    // Switch to ungrouped
    mockGroupField = undefined;
    BoardViewMock.mockClear();
    rerender(<GroupedBoardView board={board} tasks={tasks} />);

    // Flat: no section headers, single BoardView with all tasks
    expect(queryByText("alpha")).toBeNull();
    expect(queryByText("beta")).toBeNull();
    expect(getAllByTestId("board-view")).toHaveLength(1);
    expect(BoardViewMock).toHaveBeenCalledTimes(1);
    expect(BoardViewMock).toHaveBeenCalledWith(
      expect.objectContaining({ tasks }),
    );
  });

  it("handles empty task list with grouping active", () => {
    mockGroupField = "project";
    mockFieldDefs = [
      {
        id: "project",
        name: "project",
        type: { kind: "string" },
      } as import("@/types/kanban").FieldDef,
    ];
    BoardViewMock.mockClear();

    const { container } = render(<GroupedBoardView board={board} tasks={[]} />);

    // computeGroups returns [] for empty tasks, so no group sections render.
    // The grouped container is shown but with zero sections.
    expect(BoardViewMock).toHaveBeenCalledTimes(0);
    // Container should exist but be empty
    expect(container.querySelector("[class*=flex]")).toBeTruthy();
  });

  it("handles many groups without errors", () => {
    mockGroupField = "project";
    mockFieldDefs = [
      {
        id: "project",
        name: "project",
        type: { kind: "string" },
      } as import("@/types/kanban").FieldDef,
    ];
    BoardViewMock.mockClear();

    // Create 20 groups with 3 tasks each
    const tasks: Entity[] = [];
    for (let g = 0; g < 20; g++) {
      for (let t = 0; t < 3; t++) {
        tasks.push(
          makeTask(
            `t-${g}-${t}`,
            "todo",
            `group-${String(g).padStart(2, "0")}`,
          ),
        );
      }
    }

    const { container } = render(
      <GroupedBoardView board={board} tasks={tasks} />,
    );

    // Should render a BoardView per group
    expect(BoardViewMock).toHaveBeenCalledTimes(20);
    // All group headers should appear
    const buttons = container.querySelectorAll("button");
    expect(buttons.length).toBe(20);
  });
});
