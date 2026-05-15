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
  virtualTagMeta: [],
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
      <GroupSection
        bucket={bucket}
        board={board}
        groupField="tags"
        collapsed={false}
        onToggleCollapsed={() => {}}
      />,
    );
    expect(getByText("bug")).toBeTruthy();
    expect(getByText("2")).toBeTruthy();
  });

  it("renders BoardView when not collapsed", () => {
    const { getByTestId } = render(
      <GroupSection
        bucket={bucket}
        board={board}
        groupField="tags"
        collapsed={false}
        onToggleCollapsed={() => {}}
      />,
    );
    expect(getByTestId("board-view")).toBeTruthy();
  });

  it("hides BoardView when collapsed=true", () => {
    const { queryByTestId } = render(
      <GroupSection
        bucket={bucket}
        board={board}
        groupField="tags"
        collapsed={true}
        onToggleCollapsed={() => {}}
      />,
    );
    expect(queryByTestId("board-view")).toBeNull();
  });

  it("calls onToggleCollapsed when header is clicked", () => {
    const onToggleCollapsed = vi.fn();
    const { getByRole } = render(
      <GroupSection
        bucket={bucket}
        board={board}
        groupField="tags"
        collapsed={false}
        onToggleCollapsed={onToggleCollapsed}
      />,
    );

    fireEvent.click(getByRole("button", { name: /bug/i }));
    expect(onToggleCollapsed).toHaveBeenCalledTimes(1);
  });

  it("does not own collapse state — successive renders with same collapsed prop are stable", () => {
    // Regression for hoisted-state contract: a fresh `<GroupSection>`
    // mount with `collapsed={true}` must render collapsed immediately.
    // If the section secretly tracked collapse via internal `useState`,
    // a remount would reset to expanded and this assertion would fail.
    const { queryByTestId, rerender } = render(
      <GroupSection
        bucket={bucket}
        board={board}
        groupField="tags"
        collapsed={true}
        onToggleCollapsed={() => {}}
      />,
    );
    expect(queryByTestId("board-view")).toBeNull();

    // Re-render with collapsed=false — body must appear.
    rerender(
      <GroupSection
        bucket={bucket}
        board={board}
        groupField="tags"
        collapsed={false}
        onToggleCollapsed={() => {}}
      />,
    );
    expect(queryByTestId("board-view")).toBeTruthy();

    // Re-render collapsed again — body must disappear.
    rerender(
      <GroupSection
        bucket={bucket}
        board={board}
        groupField="tags"
        collapsed={true}
        onToggleCollapsed={() => {}}
      />,
    );
    expect(queryByTestId("board-view")).toBeNull();
  });

  it("section root carries data-group-section for outer-virtualizer test selectors", () => {
    const { container } = render(
      <GroupSection
        bucket={bucket}
        board={board}
        groupField="tags"
        collapsed={false}
        onToggleCollapsed={() => {}}
      />,
    );
    const section = container.querySelector("[data-group-section]");
    expect(section).toBeTruthy();
    expect(section!.getAttribute("data-group-value")).toBe("bug");
  });

  it("shows (ungrouped) label for empty-value bucket", () => {
    const ungrouped: GroupBucket = {
      value: "",
      label: "(ungrouped)",
      tasks: [makeTask("t3", "todo")],
    };
    const { getByText } = render(
      <GroupSection
        bucket={ungrouped}
        board={board}
        groupField="tags"
        collapsed={false}
        onToggleCollapsed={() => {}}
      />,
    );
    expect(getByText("(ungrouped)")).toBeTruthy();
  });
});
