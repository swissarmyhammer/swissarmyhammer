import { describe, it, expect, vi, beforeEach } from "vitest";
import { render } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import {
  EntityFocusProvider,
  useEntityFocus,
} from "@/lib/entity-focus-context";
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
    applyFilter: (entities: unknown[]) => entities,
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

/** Helper to read registered scopes from the entity focus provider. */
function ScopeProbe({
  onScope,
}: {
  onScope: (getScope: (m: string) => unknown) => void;
}) {
  const { getScope } = useEntityFocus();
  onScope(getScope);
  return null;
}

function makeColumn(id: string, name: string, order: number): Entity {
  return {
    id,
    entity_type: "column",
    fields: { name, order },
  };
}

function makeTask(id: string, columnId: string, ordinal: string): Entity {
  return {
    id,
    entity_type: "task",
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
    fields: { name: "Test Board" },
  },
  columns: [
    makeColumn("col-todo", "Todo", 0),
    makeColumn("col-doing", "Doing", 1),
    makeColumn("col-done", "Done", 2),
  ],

  tags: [],
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

  it("board nav commands are registered in scope", () => {
    let getScope: ((m: string) => unknown) | null = null;

    render(
      <EntityFocusProvider>
        <SchemaProvider>
          <EntityStoreProvider entities={{}}>
            <TooltipProvider>
              <ActiveBoardPathProvider value="/test/board">
                <DragSessionProvider>
                  <ScopeProbe
                    onScope={(fn) => {
                      getScope = fn;
                    }}
                  />
                  <BoardView board={board} tasks={tasks} />
                </DragSessionProvider>
              </ActiveBoardPathProvider>
            </TooltipProvider>
          </EntityStoreProvider>
        </SchemaProvider>
      </EntityFocusProvider>,
    );

    // The board:board scope should be registered (from FocusScope)
    expect(getScope!("board:board")).not.toBeNull();
  });
});
