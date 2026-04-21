import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

// --- Mocks ---
const mockInvoke = vi.fn(
  async (_cmd: string, _args?: unknown): Promise<unknown> => "ok",
);
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...a: unknown[]) => mockInvoke(...(a as [string, unknown?])),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}));
vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));
vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

import { ColumnView } from "./column-view";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { ActiveBoardPathProvider } from "@/lib/command-scope";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { Entity } from "@/types/kanban";

/** Create a minimal column entity. */
function makeColumn(id = "col-1", name = "To Do"): Entity {
  return {
    entity_type: "column",
    id,
    moniker: `column:${id}`,
    fields: { name },
  };
}

/** Create a minimal task entity. */
function makeTask(id: string, column = "col-1"): Entity {
  return {
    entity_type: "task",
    id,
    moniker: `task:${id}`,
    fields: {
      title: `Task ${id}`,
      position_column: column,
      position_ordinal: "a0",
    },
  };
}

/** Wrap component with required providers. */
function renderColumn(ui: React.ReactElement) {
  return render(
    <EntityFocusProvider>
      <SchemaProvider>
        <EntityStoreProvider entities={{}}>
          <TooltipProvider>
            <ActiveBoardPathProvider value="/test/board">
              {ui}
            </ActiveBoardPathProvider>
          </TooltipProvider>
        </EntityStoreProvider>
      </SchemaProvider>
    </EntityFocusProvider>,
  );
}

describe("ColumnView drop zones", () => {
  it("renders N+1 drop zones for N tasks", () => {
    const tasks = [makeTask("t1"), makeTask("t2"), makeTask("t3")];
    const { container } = renderColumn(
      <ColumnView column={makeColumn()} tasks={tasks} onDrop={vi.fn()} />,
    );

    const zones = container.querySelectorAll("[data-drop-zone]");
    expect(zones.length).toBe(4);
  });

  it("drop zones carry correct before/after attributes", () => {
    const tasks = [makeTask("t1"), makeTask("t2"), makeTask("t3")];
    const { container } = renderColumn(
      <ColumnView column={makeColumn()} tasks={tasks} onDrop={vi.fn()} />,
    );

    const zones = container.querySelectorAll("[data-drop-zone]");
    // First 3 zones are "before" zones for t1, t2, t3
    expect(zones[0].getAttribute("data-drop-before")).toBe("t1");
    expect(zones[1].getAttribute("data-drop-before")).toBe("t2");
    expect(zones[2].getAttribute("data-drop-before")).toBe("t3");
    // Last zone is "after" zone for t3
    expect(zones[3].getAttribute("data-drop-after")).toBe("t3");
  });

  it("empty column renders 1 drop zone with data-drop-empty", () => {
    const { container } = renderColumn(
      <ColumnView column={makeColumn()} tasks={[]} onDrop={vi.fn()} />,
    );

    const zones = container.querySelectorAll("[data-drop-zone]");
    expect(zones.length).toBe(1);
    expect(zones[0].hasAttribute("data-drop-empty")).toBe(true);
  });

  it("renders inert spacers for zones adjacent to the dragged task", () => {
    const tasks = [makeTask("t1"), makeTask("t2"), makeTask("t3")];
    const { container } = renderColumn(
      <ColumnView
        column={makeColumn()}
        tasks={tasks}
        dragTaskId="t2"
        onDrop={vi.fn()}
      />,
    );

    // All 4 zones still render (layout stability), but the "before t2"
    // zone is inert — it has no drag handlers, just a spacer div.
    const zones = container.querySelectorAll("[data-drop-zone]");
    expect(zones.length).toBe(4);
  });

  it("shows correct badge count", () => {
    const tasks = [makeTask("t1"), makeTask("t2")];
    renderColumn(
      <ColumnView column={makeColumn()} tasks={tasks} onDrop={vi.fn()} />,
    );

    expect(screen.getByText("2")).toBeTruthy();
  });
});

describe("ColumnView layout", () => {
  /**
   * Regression test for "columns don't fill the available vertical space".
   *
   * The column scope (`data-moniker="column:<id>"`) is a `flex flex-col`
   * container whose children must be: (1) a content-sized header and (2) a
   * `flex-1` card list that consumes all remaining vertical space.
   *
   * If the header is ALSO given `flex-1` (as it was before this fix), the
   * two `flex-1` siblings split the space 50/50 and the card list visually
   * occupies only the bottom half of the column.
   *
   * jsdom does not lay out flexbox, so we assert on class structure instead
   * of measured rects: the card list must carry `flex-1`, and no
   * non-card-list direct child of the column scope may carry `flex-1`.
   */
  it("card list is the only flex-1 child of the column scope", () => {
    const tasks = [makeTask("t1"), makeTask("t2"), makeTask("t3")];
    renderColumn(
      <ColumnView column={makeColumn()} tasks={tasks} onDrop={vi.fn()} />,
    );

    const scope = document.querySelector(
      '[data-moniker="column:col-1"]',
    ) as HTMLElement | null;
    expect(scope).toBeTruthy();

    // Find the card list — it's the one with overflow-y-auto.
    const cardList = scope!.querySelector(
      "div.overflow-y-auto",
    ) as HTMLElement | null;
    expect(cardList).toBeTruthy();
    expect(cardList!.className).toContain("flex-1");

    // No direct child of the column scope other than the card list may
    // claim `flex-1` — otherwise it competes with the card list for space.
    for (const child of Array.from(scope!.children) as HTMLElement[]) {
      if (child === cardList) continue;
      // `flex-1` is a whole token; reject it as a class but allow substrings
      // like `flex-col` that merely start with "flex-".
      const classes = child.className.split(/\s+/);
      expect(classes).not.toContain("flex-1");
    }
  });

  it("column header row is a direct child of the column scope", () => {
    // After removing the redundant `flex-col flex-1` wrapper, the
    // `.column-header-focus` row (the actual header content) should be a
    // direct child of the column scope so it sizes to its content height.
    renderColumn(
      <ColumnView column={makeColumn()} tasks={[]} onDrop={vi.fn()} />,
    );

    const scope = document.querySelector(
      '[data-moniker="column:col-1"]',
    ) as HTMLElement | null;
    expect(scope).toBeTruthy();

    const header = scope!.querySelector(
      ".column-header-focus",
    ) as HTMLElement | null;
    expect(header).toBeTruthy();
    expect(header!.parentElement).toBe(scope);
  });
});

describe("ColumnView add-task button", () => {
  it("has aria-label with column name and no title attribute", () => {
    renderColumn(
      <ColumnView
        column={makeColumn("col-1", "To Do")}
        tasks={[]}
        onAddTask={vi.fn()}
        onDrop={vi.fn()}
      />,
    );

    const btn = screen.getByRole("button", { name: /add task to to do/i });
    expect(btn).toBeTruthy();
    expect(btn.getAttribute("title")).toBeNull();
  });

  it("calls onAddTask with column id when clicked", () => {
    const onAddTask = vi.fn();
    renderColumn(
      <ColumnView
        column={makeColumn("col-1", "To Do")}
        tasks={[]}
        onAddTask={onAddTask}
        onDrop={vi.fn()}
      />,
    );

    const btn = screen.getByRole("button", { name: /add task to to do/i });
    fireEvent.click(btn);
    expect(onAddTask).toHaveBeenCalledWith("col-1");
  });

  it("does not render add button when onAddTask is not provided", () => {
    renderColumn(
      <ColumnView
        column={makeColumn("col-1", "To Do")}
        tasks={[]}
        onDrop={vi.fn()}
      />,
    );

    expect(screen.queryByRole("button", { name: /add task/i })).toBeNull();
  });
});

describe("ColumnView — Do This Next command", () => {
  afterEach(() => {
    mockInvoke.mockReset();
    mockInvoke.mockImplementation(async () => "ok");
  });

  it("context menu dispatches task.doThisNext through the backend, not task.move", async () => {
    mockInvoke.mockImplementation(
      async (cmd: string, args?: unknown): Promise<unknown> => {
        if (cmd === "list_entity_types") return ["task", "column"];
        if (cmd === "get_entity_schema") {
          const a = args as { entityType: string } | undefined;
          if (a?.entityType === "task") {
            return {
              entity: {
                name: "task",
                fields: ["title", "position_column", "position_ordinal"],
                commands: [
                  {
                    id: "task.doThisNext",
                    name: "Do This Next",
                    context_menu: true,
                  },
                ],
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
          }
          return {
            entity: {
              name: a?.entityType ?? "unknown",
              fields: ["name"],
              commands: [],
            },
            fields: [
              {
                id: "name",
                name: "name",
                type: { kind: "text" },
                section: "header",
                display: "text",
                editor: "text",
              },
            ],
          };
        }
        if (cmd === "list_commands_for_scope") {
          return [
            {
              id: "task.doThisNext",
              name: "Do This Next",
              target: "task:t1",
              group: "entity",
              context_menu: true,
              available: true,
            },
          ];
        }
        if (cmd === "show_context_menu") return undefined;
        return "ok";
      },
    );

    const task = makeTask("t1");
    renderColumn(
      <ColumnView column={makeColumn()} tasks={[task]} onDrop={vi.fn()} />,
    );

    // Right-click the task card's FocusScope
    const taskScope = document.querySelector(
      "[data-moniker='task:t1']",
    ) as HTMLElement | null;
    expect(taskScope).toBeTruthy();
    fireEvent.contextMenu(taskScope!);

    // Assert show_context_menu is called with task.doThisNext
    await waitFor(() => {
      const showCall = mockInvoke.mock.calls.find(
        (c: unknown[]) => c[0] === "show_context_menu",
      );
      expect(showCall).toBeTruthy();
      const items = (showCall![1] as { items: { cmd: string }[] }).items;
      const doThisNext = items.find(
        (i: { cmd: string }) => i.cmd === "task.doThisNext",
      );
      expect(doThisNext).toBeTruthy();
    });

    // Assert task.move was NOT dispatched (old workaround is gone)
    const moveDispatch = mockInvoke.mock.calls.find(
      (c: unknown[]) =>
        c[0] === "dispatch_command" &&
        (c[1] as { cmd?: string })?.cmd === "task.move",
    );
    expect(moveDispatch).toBeUndefined();
  });

  it("context menu scope chain contains the task moniker", async () => {
    mockInvoke.mockImplementation(
      async (cmd: string, _args?: unknown): Promise<unknown> => {
        if (cmd === "list_commands_for_scope") {
          return [
            {
              id: "task.doThisNext",
              name: "Do This Next",
              target: "task:t2",
              group: "entity",
              context_menu: true,
              available: true,
            },
          ];
        }
        if (cmd === "show_context_menu") return undefined;
        return "ok";
      },
    );

    const task = makeTask("t2");
    renderColumn(
      <ColumnView column={makeColumn()} tasks={[task]} onDrop={vi.fn()} />,
    );

    const taskScope = document.querySelector(
      "[data-moniker='task:t2']",
    ) as HTMLElement | null;
    expect(taskScope).toBeTruthy();
    fireEvent.contextMenu(taskScope!);

    await waitFor(() => {
      const showCall = mockInvoke.mock.calls.find(
        (c: unknown[]) => c[0] === "show_context_menu",
      );
      expect(showCall).toBeTruthy();
      const items = (
        showCall![1] as { items: { cmd: string; scope_chain: string[] }[] }
      ).items;
      const doThisNext = items.find(
        (i: { cmd: string }) => i.cmd === "task.doThisNext",
      );
      expect(doThisNext).toBeTruthy();
      expect(doThisNext!.scope_chain).toContain("task:t2");
    });
  });

  it("DraggableTaskCard receives no extraCommands from column (re-render stability)", () => {
    // After deleting the buildDoThisNextCommand workaround, the column no
    // longer passes extraCommands to DraggableTaskCard. The prop is always
    // undefined, so React.memo on DraggableTaskCard never sees a changed
    // reference — sibling cards skip re-rendering when one task moves.
    const tasks = [
      makeTask("t1"),
      makeTask("t2"),
      makeTask("t3"),
      makeTask("t4"),
      makeTask("t5"),
    ];
    const { container } = renderColumn(
      <ColumnView column={makeColumn()} tasks={tasks} onDrop={vi.fn()} />,
    );

    // All 5 cards render
    const cards = container.querySelectorAll("[data-entity-card]");
    expect(cards.length).toBe(5);

    // Verify column-view no longer injects extraCommands: the workaround
    // function buildDoThisNextCommand and the taskExtraCommands map have
    // been deleted. This means the VirtualizedCardList and VirtualColumn
    // components receive no taskExtraCommands prop, and DraggableTaskCard
    // receives no extraCommands — the prop is stably undefined, allowing
    // React.memo to skip re-renders when the entity reference is unchanged.
    // The absence of dispatchTaskMove (useDispatchCommand("task.move"))
    // confirms no frontend-side task.move dispatch is wired.
    const dispatchCalls = mockInvoke.mock.calls.filter(
      (c: unknown[]) => c[0] === "dispatch_command",
    );
    const moveDispatches = dispatchCalls.filter(
      (c: unknown[]) => (c[1] as { cmd?: string })?.cmd === "task.move",
    );
    expect(moveDispatches.length).toBe(0);
  });
});
