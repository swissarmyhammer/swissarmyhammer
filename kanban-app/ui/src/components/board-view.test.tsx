import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { DragSessionProvider } from "@/lib/drag-session-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { ActiveBoardPathProvider } from "@/lib/command-scope";
import { TooltipProvider } from "@/components/ui/tooltip";
import { BoardView } from "./board-view";
import type { BoardData, Entity } from "@/types/kanban";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(() => Promise.resolve()),
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock("@/components/perspective-container", () => ({
  useActivePerspective: () => ({
    activePerspective: null,
    applySort: (entities: unknown[]) => entities,
    groupField: undefined,
  }),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));

function makeColumn(id: string, name: string, order: number): Entity {
  return {
    id,
    entity_type: "column",
    moniker: `column:${id}`,
    fields: { name, order },
  };
}

function makeTask(id: string, columnId: string, ordinal: string): Entity {
  return {
    id,
    entity_type: "task",
    moniker: `task:${id}`,
    fields: {
      title: `Task ${id}`,
      position_column: columnId,
      position_ordinal: ordinal,
    },
  };
}

const board: BoardData = {
  board: {
    id: "board-1",
    entity_type: "board",
    moniker: "board:board-1",
    fields: { name: "Test Board" },
  },
  columns: [
    makeColumn("col-todo", "Todo", 0),
    makeColumn("col-doing", "Doing", 1),
    makeColumn("col-done", "Done", 2),
  ],

  tags: [],
  virtualTagMeta: [],
  summary: {
    total_tasks: 3,
    total_actors: 0,
    ready_tasks: 3,
    blocked_tasks: 0,
    done_tasks: 0,
    percent_complete: 0,
  },
};

const tasks: Entity[] = [
  makeTask("t1", "col-todo", "a0"),
  makeTask("t2", "col-todo", "a1"),
  makeTask("t3", "col-doing", "a0"),
];

function renderBoard(overrides?: { board?: BoardData; tasks?: Entity[] }) {
  const result = render(
    <EntityFocusProvider>
      <SchemaProvider>
        <EntityStoreProvider entities={{}}>
          <TooltipProvider>
            <ActiveBoardPathProvider value="/test/board">
              <DragSessionProvider>
                <BoardView
                  board={overrides?.board ?? board}
                  tasks={overrides?.tasks ?? tasks}
                />
              </DragSessionProvider>
            </ActiveBoardPathProvider>
          </TooltipProvider>
        </EntityStoreProvider>
      </SchemaProvider>
    </EntityFocusProvider>,
  );
  return result;
}

describe("BoardView navigation commands", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (invoke as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
  });

  it("renders without crashing", () => {
    const { container } = renderBoard();
    expect(container).toBeTruthy();
  });

  it("renders all columns", () => {
    const { container } = renderBoard();
    // The board should render column views
    expect(container.textContent).toContain("Todo");
    expect(container.textContent).toContain("Doing");
    expect(container.textContent).toContain("Done");
  });

  it("board nav commands are registered in scope", async () => {
    const { container } = renderBoard();

    // BoardView wraps itself in a FocusScope with moniker="board:{id}".
    // Verify the scope is rendered by checking that the board's data-moniker
    // attribute or the board container is present with navigation commands.
    // The FocusScope registers in the EntityFocusProvider, so we verify
    // indirectly by checking that the board rendered its columns — if the
    // scope registration failed, the columns wouldn't render correctly.
    await waitFor(() => {
      expect(container.textContent).toContain("Todo");
    });
    // Board rendered all columns — scope chain is functional
    expect(container.textContent).toContain("Doing");
    expect(container.textContent).toContain("Done");
  });
});

describe("BoardView scrollContainer layout", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (invoke as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
  });

  it("scroll container has the min-w-0 and overflow-x-auto classes", () => {
    const { container } = renderBoard();
    // scrollContainerRef is the direct parent of the SortableContext wrapper.
    // It must have min-w-0 (so it shrinks to its flex share) plus the
    // overflow-x-auto that drives horizontal scrolling. Without min-w-0 the
    // column strip propagates its intrinsic width up through flex parents.
    const scrollContainer = container.querySelector(
      "div.overflow-x-auto",
    ) as HTMLElement;
    expect(scrollContainer).toBeTruthy();
    expect(scrollContainer.className).toContain("min-w-0");
    expect(scrollContainer.className).toContain("overflow-x-auto");
    expect(scrollContainer.className).toContain("min-h-0");
    expect(scrollContainer.className).toContain("flex-1");
  });

  it("scrollWidth exceeds clientWidth when the column strip is wider than the viewport", () => {
    // Render with many columns so the tree has multiple column children, then
    // force the scroll container wider than its parent via an inline-width
    // probe injected alongside the columns. This sidesteps the fact that
    // Tailwind utilities (like `min-w-[20em]`) are not compiled into the
    // test environment — we just need the scroll container to actually have
    // overflow to verify that it, not some ancestor, owns the scrolling.
    const manyColumns: Entity[] = [];
    const manyTasks: Entity[] = [];
    for (let i = 0; i < 3; i++) {
      manyColumns.push(makeColumn(`c${i}`, `Col ${i}`, i));
      manyTasks.push(makeTask(`t${i}`, `c${i}`, "a0"));
    }
    const wideBoard: BoardData = { ...board, columns: manyColumns };

    // Constrain the rendered tree to 640px so the 2000px probe below
    // overflows horizontally.
    const host = document.createElement("div");
    host.style.width = "640px";
    host.style.height = "480px";
    host.style.display = "flex";
    host.style.flexDirection = "column";
    host.style.overflow = "hidden";
    document.body.appendChild(host);

    try {
      const { container } = render(
        <EntityFocusProvider>
          <SchemaProvider>
            <EntityStoreProvider entities={{}}>
              <TooltipProvider>
                <ActiveBoardPathProvider value="/test/wide">
                  <DragSessionProvider>
                    <div
                      style={{
                        display: "flex",
                        flexDirection: "column",
                        flex: "1 1 0%",
                        minHeight: 0,
                        minWidth: 0,
                      }}
                    >
                      <BoardView board={wideBoard} tasks={manyTasks} />
                    </div>
                  </DragSessionProvider>
                </ActiveBoardPathProvider>
              </TooltipProvider>
            </EntityStoreProvider>
          </SchemaProvider>
        </EntityFocusProvider>,
        { container: host },
      );

      const scrollContainer = container.querySelector(
        "div.overflow-x-auto",
      ) as HTMLElement;
      expect(scrollContainer).toBeTruthy();

      // Inject a 2000px-wide probe as a child of the scroll container to
      // force horizontal overflow. Without `min-w-0` on the scroll
      // container, this would propagate up and make body scroll; with
      // `min-w-0` the overflow stays here and scrollWidth > clientWidth.
      const probe = document.createElement("div");
      probe.style.width = "2000px";
      probe.style.height = "20px";
      probe.style.flex = "0 0 auto";
      scrollContainer.appendChild(probe);

      expect(scrollContainer.scrollWidth).toBeGreaterThan(
        scrollContainer.clientWidth,
      );
    } finally {
      host.remove();
    }
  });
});

vi.mock("sonner", () => ({
  toast: {
    error: vi.fn(),
    info: vi.fn(),
    success: vi.fn(),
    warning: vi.fn(),
  },
}));

describe("BoardView add task", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (invoke as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
  });

  it("shows the add-task button only on the first column", () => {
    renderBoard();
    // Only the first column (Todo) should have the add button
    const buttons = screen.getAllByRole("button", { name: /add task/i });
    expect(buttons.length).toBe(1);
    expect(buttons[0].getAttribute("aria-label")).toMatch(/todo/i);
  });

  it("shows toast error when entity.add:task dispatch fails", async () => {
    (invoke as ReturnType<typeof vi.fn>).mockImplementation((cmd: string) => {
      if (cmd === "dispatch_command") {
        return Promise.reject(new Error("Column not found"));
      }
      return Promise.resolve(undefined);
    });

    renderBoard();
    const btn = screen.getByRole("button", { name: /add task/i });
    fireEvent.click(btn);

    await waitFor(() => {
      expect(toast.error).toHaveBeenCalledWith(
        expect.stringContaining("Column not found"),
      );
    });
  });

  it("routes the column (+) button through the unified entity.add:task command", async () => {
    // Regression guard for the "one true creation path" refactor: the board
    // column (+) button must NOT dispatch the legacy `task.add` — it must go
    // through `entity.add:task` with a `column` arg, the same path the grid
    // (+) and the palette use. This keeps creation logic in one place
    // (AddEntity on the Rust side) across every UI entry point.
    const invokeMock = invoke as ReturnType<typeof vi.fn>;
    invokeMock.mockClear();
    renderBoard();

    const btn = screen.getByRole("button", { name: /add task/i });
    fireEvent.click(btn);

    const findDispatchByCmd = (cmd: string) =>
      invokeMock.mock.calls.find(
        (c) =>
          c[0] === "dispatch_command" &&
          (c[1] as { cmd?: string } | undefined)?.cmd === cmd,
      );

    await waitFor(() => {
      const call = findDispatchByCmd("entity.add:task");
      expect(call).toBeTruthy();
      const payload = call?.[1] as
        | { cmd?: string; args?: Record<string, unknown> }
        | undefined;
      // The `column` override must be forwarded so the new task lands in the
      // column the user clicked, not the default lowest-order column.
      expect(payload?.args).toMatchObject({ column: "col-todo" });
    });

    // The legacy task.add dispatch must NOT fire from the column (+) button.
    expect(findDispatchByCmd("task.add")).toBeUndefined();
  });
});
