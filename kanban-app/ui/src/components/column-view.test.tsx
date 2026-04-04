import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";

// --- Mocks ---
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve("ok")),
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
  return { entity_type: "column", id, fields: { name } };
}

/** Create a minimal task entity. */
function makeTask(id: string, column = "col-1"): Entity {
  return {
    entity_type: "task",
    id,
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
