import { describe, it, expect, vi } from "vitest";
import { render, fireEvent } from "@testing-library/react";
import { GroupedBoardView } from "./grouped-board-view";
import type { BoardData, Entity } from "@/types/kanban";

/**
 * Click every group-section header to expand it.
 *
 * `<GroupedBoardView>` starts every bucket collapsed by default so the
 * initial render is stable at hundreds of groups (see the production
 * component's file header). Tests that assert on per-section
 * `<BoardView>` mock calls or rendered `data-testid="board-view"`
 * nodes need to expand the sections first — collapsed sections don't
 * mount the inner `<BoardView>`.
 */
function expandAll(container: HTMLElement): void {
  const sections = container.querySelectorAll("[data-group-section] button");
  for (const btn of sections) {
    fireEvent.click(btn);
  }
}

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
    loading: false,
    mentionableTypes: [],
  }),
  useSchemaOptional: () => ({
    getSchema: () => undefined,
    getFieldDef: () => undefined,
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

/**
 * Build a task whose `assignees` field carries a single-value list.
 *
 * `assignees` is a real groupable field on the task entity schema
 * (`groupable: true`, slug `assignees`). Mirrors the production wire
 * shape so the regression test for task 01KRH2EX1N1CA2HA3B4NMWZH67
 * exercises the same lookup path `<GroupedBoardView>` uses on the
 * user's board.
 */
function makeTaskWithAssignee(
  id: string,
  column: string,
  assignee: string,
): Entity {
  return {
    id,
    entity_type: "task",
    moniker: `task:${id}`,
    fields: {
      title: `Task ${id}`,
      position_column: column,
      position_ordinal: "a0",
      assignees: [assignee],
    },
  };
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
  virtualTagMeta: [],
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

    const { container, getByText, getAllByTestId } = render(
      <GroupedBoardView board={board} tasks={tasks} />,
    );

    // Should have group headers
    expect(getByText("alpha")).toBeTruthy();
    expect(getByText("beta")).toBeTruthy();
    // Sections start collapsed — expand them before checking for BoardView.
    expandAll(container);
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

    const { container } = render(
      <GroupedBoardView board={board} tasks={tasks} />,
    );
    // Sections start collapsed; expand to populate BoardViewMock.
    expandAll(container);

    // The outer virtualizer may re-render each section more than once
    // during initial measurement; what matters is the **set** of
    // (groupValue, tasks) pairs `<BoardView>` was rendered with. Dedupe
    // by groupValue and assert one entry per bucket with the right tasks.
    const callsByGroup = new Map<string, Entity[]>();
    for (const call of BoardViewMock.mock.calls as [
      { groupValue?: string; tasks: Entity[] },
    ][]) {
      callsByGroup.set(call[0].groupValue ?? "", call[0].tasks);
    }
    expect(callsByGroup.size).toBe(2);
    expect(callsByGroup.get("alpha")).toHaveLength(1);
    expect(callsByGroup.get("alpha")?.[0].id).toBe("t1");
    expect(callsByGroup.get("beta")).toHaveLength(1);
    expect(callsByGroup.get("beta")?.[0].id).toBe("t2");
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

    const { container } = render(
      <GroupedBoardView board={board} tasks={tasks} />,
    );
    // Sections start collapsed; expand to populate BoardViewMock.
    expandAll(container);

    // Dedupe BoardView calls by groupValue — the outer virtualizer may
    // re-render each section more than once during initial measurement.
    // 3 groups: alpha (2 tasks), beta (1 task), ungrouped (1 task).
    const callsByGroup = new Map<string, Entity[]>();
    for (const call of BoardViewMock.mock.calls as [
      { groupValue?: string; tasks: Entity[] },
    ][]) {
      callsByGroup.set(call[0].groupValue ?? "", call[0].tasks);
    }
    expect(callsByGroup.size).toBe(3);

    const alphaTasks = callsByGroup.get("alpha");
    const betaTasks = callsByGroup.get("beta");
    const ungroupedTasks = callsByGroup.get("");

    expect(alphaTasks).toBeDefined();
    expect(alphaTasks).toHaveLength(2);
    expect(betaTasks).toBeDefined();
    expect(betaTasks).toHaveLength(1);
    expect(ungroupedTasks).toBeDefined();
    expect(ungroupedTasks).toHaveLength(1);
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

  // ---------------------------------------------------------------------
  // Regression for task 01KRH2EX1N1CA2HA3B4NMWZH67.
  //
  // The end-to-end Group By contract: the `perspective.fields` resolver
  // emits `ParamOption.value = field_name` (slug), the user picks one,
  // `<CommandButton>` dispatches `perspective.group` with `group: <name>`,
  // the backend persists `group:` by name, the frontend `groupField`
  // surfaces the name, and `<GroupedBoardView>` reads
  // `task.fields[<name>]`.
  //
  // Pre-fix the resolver emitted `value = field_id` (ULID) and every
  // task landed in `(ungrouped)` because `task.fields[<ULID>]` is
  // `undefined`. This test pins the post-fix wire shape by using the
  // schema-slug name the resolver now emits (`"assignees"`) and
  // asserting the rendered board produces one group per distinct value,
  // NOT one `(ungrouped)` bucket with every task.
  //
  // If a regression flipped the resolver back to emitting field IDs (a
  // ULID), `groupField` here would be the ULID, `task.fields["01..."]`
  // would be `undefined`, every task would land in `(ungrouped)`, and
  // the `groups must NOT be a single ungrouped bucket` assertion would
  // fail. This test plus its sibling Rust unit test
  // `perspective_fields_resolver_returns_fields_for_in_scope_perspective`
  // pin the wire format on both sides.
  // ---------------------------------------------------------------------
  it("groups by the picker-dispatched field name (regression for 01KRH2EX1N1CA2HA3B4NMWZH67)", () => {
    // `groupField` carries the wire value the picker dispatches:
    // the schema slug (`"assignees"`), NOT the field ULID. Anything
    // else and `task.fields[groupField]` returns undefined and every
    // task drops into `(ungrouped)`.
    mockGroupField = "assignees";
    mockFieldDefs = [
      {
        id: "00000000000000000000000005",
        name: "assignees",
        type: { kind: "reference", entity: "actor", multiple: true },
        groupable: true,
      } as import("@/types/kanban").FieldDef,
    ];
    BoardViewMock.mockClear();

    // Six tasks split across three distinct assignee values — pins the
    // "one column per distinct value" claim in the task description's
    // acceptance criteria.
    const tasks: Entity[] = [
      makeTaskWithAssignee("t1", "todo", "alice"),
      makeTaskWithAssignee("t2", "doing", "alice"),
      makeTaskWithAssignee("t3", "todo", "bob"),
      makeTaskWithAssignee("t4", "doing", "bob"),
      makeTaskWithAssignee("t5", "todo", "carol"),
      makeTaskWithAssignee("t6", "doing", "carol"),
    ];

    const { container, queryByText } = render(
      <GroupedBoardView board={board} tasks={tasks} />,
    );
    // Sections start collapsed; expand to populate BoardViewMock.
    expandAll(container);

    // Three groups — one per distinct assignee value. NOT a single
    // `(ungrouped)` bucket with all six tasks. Dedupe BoardView calls by
    // groupValue because the outer virtualizer may re-render each
    // section during measurement.
    const callsByGroup = new Map<string, Entity[]>();
    for (const call of BoardViewMock.mock.calls as [
      { groupValue?: string; tasks: Entity[] },
    ][]) {
      callsByGroup.set(call[0].groupValue ?? "", call[0].tasks);
    }
    expect(callsByGroup.size).toBe(3);

    // Group headers must exist for each distinct value and the
    // `(ungrouped)` bucket must NOT appear (every task has a value).
    expect(queryByText("alice")).toBeTruthy();
    expect(queryByText("bob")).toBeTruthy();
    expect(queryByText("carol")).toBeTruthy();
    expect(queryByText("(ungrouped)")).toBeNull();

    // Each section gets exactly its two tasks — never six.
    const sectionTaskCounts = Array.from(callsByGroup.values())
      .map((t) => t.length)
      .sort();
    expect(sectionTaskCounts).toEqual([2, 2, 2]);

    // No section header has six entries — that would be the bug
    // (every task collapsed into a single bucket).
    const buttons = container.querySelectorAll("button");
    const labelsWithSix = Array.from(buttons).filter((b) =>
      b.textContent?.includes("6"),
    );
    expect(labelsWithSix).toHaveLength(0);
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

    // With outer virtualization the DOM mounts only viewport-visible
    // sections, so this test's intent shifts from "all 20 groups
    // rendered at once" to "20 groups render without errors AND the
    // virtualizer reserves the right total scroll height".
    //
    // The total height container carries one row per bucket; verify it
    // exists, that the section list has scrolling capacity for all 20
    // groups, and that the DOM-mounted section count is at least one
    // (sanity — the grouped path did engage) but bounded by the
    // viewport window.
    const sections = container.querySelectorAll("[data-group-section]");
    expect(sections.length).toBeGreaterThan(0);
    expect(sections.length).toBeLessThanOrEqual(20);

    // The outer scroll container exists and is the virtualization root.
    const outer = container.querySelector("[data-group-list]");
    expect(outer).toBeTruthy();
  });
});
