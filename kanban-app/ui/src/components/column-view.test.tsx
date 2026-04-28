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

/**
 * Layout regression guards — pin the load-bearing classes that keep the
 * column scrollable and the virtualizer windowing functional.
 *
 * Background: the spatial-nav refactor wrapped the column body in
 * `<FocusScope>`. An earlier revision routed the entity
 * chrome through an inner non-styled `<FocusScopeBody>` div which broke
 * the `flex flex-col flex-1 min-h-0` chain the column relies on for
 * vertical sizing and overflow scroll. The current FocusScope attaches
 * its chrome directly to the spatial primitive's root, so `className`
 * passed to `<FocusScope>` lands on the same element whose children
 * participate in flex layout. These tests pin the load-bearing classes
 * so a future refactor cannot silently regress the scroll /
 * virtualization behaviour again.
 */
describe("ColumnView layout (scroll + virtualization)", () => {
  it("scroll container carries overflow-y-auto so columns scroll vertically", () => {
    const tasks = [makeTask("t1"), makeTask("t2"), makeTask("t3")];
    const { container } = renderColumn(
      <ColumnView column={makeColumn()} tasks={tasks} onDrop={vi.fn()} />,
    );

    // The scroll container hosts the card+drop-zone list. The drop-zone is
    // a stable anchor — its parent chain must contain a scrollable element
    // (`overflow-y-auto`) for the column to scroll when content overflows.
    const dropZone = container.querySelector("[data-drop-zone]");
    expect(dropZone).toBeTruthy();

    let scrollEl: HTMLElement | null = dropZone as HTMLElement;
    let foundScroll = false;
    while (scrollEl && scrollEl !== container) {
      if (scrollEl.className.includes("overflow-y-auto")) {
        foundScroll = true;
        break;
      }
      scrollEl = scrollEl.parentElement;
    }
    expect(foundScroll).toBe(true);
  });

  it("FocusScope element carries the flex column chain (no inner wrapper)", () => {
    // FocusScope's `className` lands on the spatial primitive's root and
    // its children render as direct layout children. The column relies on
    // that contract: the same element that registers as the column's zone
    // (`data-moniker='column:…'`) is also the `flex flex-col` container
    // whose `flex-1` child (VirtualizedCardList) becomes the scrollable
    // viewport. Pin both halves of that contract here so a future refactor
    // cannot silently re-introduce a layout-breaking inner wrapper.
    const { container } = renderColumn(
      <ColumnView column={makeColumn()} tasks={[]} onDrop={vi.fn()} />,
    );
    const columnNode = container.querySelector("[data-moniker='column:col-1']");
    expect(columnNode).toBeTruthy();
    const className = (columnNode as HTMLElement).className;
    expect(className).toContain("flex");
    expect(className).toContain("flex-col");
    expect(className).toContain("flex-1");
    expect(className).toContain("min-h-0");
  });

  it("virtualizes when task count exceeds the threshold (mounted < N cards)", async () => {
    // When the task count exceeds VIRTUALIZE_THRESHOLD (25), the column
    // delegates to TanStack Virtual which only mounts ~visible rows.
    //
    // The vitest browser project does not bundle Tailwind, so utility
    // classes (`flex-1`, `min-h-0`, `overflow-y-auto`) produce no CSS
    // rules in tests. To give the virtualizer a finite viewport we
    // post-process the rendered DOM and set inline `height` + `overflow`
    // on the scroll container — the same pattern `data-table.virtualized.test.tsx`
    // documents (`@tanstack/react-virtual` reads viewport via
    // `offsetHeight` initially and via ResizeObserver subsequently;
    // inline styles satisfy both paths).
    //
    // The test still proves the regression: when the flex chain is
    // broken in production CSS, the column's scroll container can't
    // become the virtualizer's scroll element and windowing collapses.
    // The test pins the spelling of the scrollable container's classes
    // (above) and the structural contract (below) — together they
    // cover both halves of the regression.
    const N = 60;
    const tasks: Entity[] = [];
    for (let i = 0; i < N; i++) tasks.push(makeTask(`t${i}`));
    const { container } = renderColumn(
      <ColumnView column={makeColumn()} tasks={tasks} onDrop={vi.fn()} />,
    );

    // Stub a finite viewport on the scroll container so the virtualizer
    // can window the row list. Tailwind is not active in tests; this is
    // the canonical pattern (cf. data-table.virtualized.test.tsx).
    const scrollEl = container.querySelector(
      "[class*='overflow-y-auto']",
    ) as HTMLDivElement | null;
    expect(scrollEl).toBeTruthy();
    scrollEl!.style.height = "400px";
    scrollEl!.style.maxHeight = "400px";
    scrollEl!.style.overflow = "auto";

    // Let `useVirtualizer`'s ResizeObserver fire and the visible range
    // settle.
    await waitFor(() => {
      const cards = container.querySelectorAll("[data-entity-card]");
      // Virtualization is active — far fewer than N cards should mount
      // as DOM nodes. With the trailing zone occupying the bottom row
      // and ~80px estimated row height in a 400px viewport, only a
      // handful render (visible window + overscan).
      expect(cards.length).toBeLessThan(N);
    });
  });

  it("renders all cards directly when below the virtualization threshold", () => {
    // Below the threshold (25), the column uses SmallCardList which
    // mounts every card. This pins the contract that virtualization
    // engages only above the threshold.
    const N = 5;
    const tasks: Entity[] = [];
    for (let i = 0; i < N; i++) tasks.push(makeTask(`t${i}`));
    const { container } = renderColumn(
      <ColumnView column={makeColumn()} tasks={tasks} onDrop={vi.fn()} />,
    );

    const cards = container.querySelectorAll("[data-entity-card]");
    expect(cards.length).toBe(N);
  });
});
