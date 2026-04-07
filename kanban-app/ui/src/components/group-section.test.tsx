import { describe, it, expect, vi } from "vitest";
import { render, fireEvent } from "@testing-library/react";
import { GroupSection } from "./group-section";
import type { GroupBucket } from "@/lib/group-utils";
import type { BoardData, Entity } from "@/types/kanban";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(() => Promise.resolve()),
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock("@/components/board-view", () => ({
  BoardView: ({ tasks }: { board: BoardData; tasks: Entity[] }) => (
    <div data-testid="board-view">
      {tasks.map((t) => (
        <div key={t.id} data-testid={`task-${t.id}`}>
          {String(t.fields.title)}
        </div>
      ))}
    </div>
  ),
}));

function makeTask(id: string, column: string): Entity {
  return {
    id,
    entity_type: "task",
    moniker: `task:${id}`,
    fields: { title: `Task ${id}`, position_column: column },
  };
}

const board: BoardData = {
  board: {
    id: "b1",
    entity_type: "board",
    moniker: "board:b1",
    fields: { name: "Board" },
  },
  columns: [
    {
      id: "todo",
      entity_type: "column",
      moniker: "column:todo",
      fields: { name: "Todo", order: 0 },
    },
  ],
  tags: [],
  summary: {
    total_tasks: 2,
    total_actors: 0,
    ready_tasks: 2,
    blocked_tasks: 0,
    done_tasks: 0,
    percent_complete: 0,
  },
};

const bucket: GroupBucket = {
  value: "bug",
  label: "bug",
  tasks: [makeTask("t1", "todo"), makeTask("t2", "todo")],
};

describe("GroupSection", () => {
  it("renders group label and task count", () => {
    const { getByText } = render(
      <GroupSection bucket={bucket} board={board} groupField="tags" />,
    );
    expect(getByText("bug")).toBeTruthy();
    expect(getByText("2")).toBeTruthy();
  });

  it("renders BoardView when expanded", () => {
    const { getByTestId } = render(
      <GroupSection bucket={bucket} board={board} groupField="tags" />,
    );
    expect(getByTestId("board-view")).toBeTruthy();
  });

  it("collapses when header is clicked", () => {
    const { getByRole, queryByTestId } = render(
      <GroupSection bucket={bucket} board={board} groupField="tags" />,
    );
    // Initially expanded
    expect(queryByTestId("board-view")).toBeTruthy();

    // Click header to collapse
    fireEvent.click(getByRole("button", { name: /bug/i }));
    expect(queryByTestId("board-view")).toBeNull();

    // Click again to expand
    fireEvent.click(getByRole("button", { name: /bug/i }));
    expect(queryByTestId("board-view")).toBeTruthy();
  });

  it("shows (ungrouped) label for empty-value bucket", () => {
    const ungrouped: GroupBucket = {
      value: "",
      label: "(ungrouped)",
      tasks: [makeTask("t3", "todo")],
    };
    const { getByText } = render(
      <GroupSection bucket={ungrouped} board={board} groupField="tags" />,
    );
    expect(getByText("(ungrouped)")).toBeTruthy();
  });
});
