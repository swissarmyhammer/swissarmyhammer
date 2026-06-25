/**
 * Browser-mode test for `<BoardView>`'s `board.newTask` key bindings —
 * vim `o` and cua `Mod+Enter`.
 *
 * **Card F behaviour** (this test pins): `board.newTask` is DEFINED by the
 * `board-commands` builtin plugin (`builtin/plugins/board-commands/index.ts`,
 * scope-gated to the `ui:board` marker the board view mounts), and its live
 * BEHAVIOR is a webview-bus handler the board registers on mount
 * (`registerWebviewCommandHandler`, Card B). The handler is pure
 * orchestration:
 *
 *   1. The DURABLE add re-dispatches the backend-op `entity.add:task`
 *      command through `useDispatchCommand` — never an inline mutation
 *      (the presentation-only bus invariant). The backend resolves the
 *      target column from the dispatch scope chain
 *      (`resolve_focused_column`).
 *   2. On success it focuses the created card by dispatching `nav.focus`
 *      with the composed card FQM.
 *
 * Because the bus handler intercepts the id in `useDispatchCommand`, the
 * `board.newTask` id itself never reaches the backend.
 *
 * Sister test to `board-view.column-extremes.spatial.test.tsx` — same
 * harness, same mock pattern, same browser project.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act, fireEvent } from "@testing-library/react";
import {
  answerListCommand,
  globalCommandsFromBindingTables,
} from "@/test/mock-command-list";
import type { BoardData, Entity } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before component imports.
// ---------------------------------------------------------------------------

const { mockInvoke, mockListen, listeners } = await vi.hoisted(async () => {
  const { setupSpatialMocks } = await import("@/test/spatial-nav-harness");
  return setupSpatialMocks();
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...a: unknown[]) => mockInvoke(...(a as [string, unknown?])),
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(() => Promise.resolve()),
  listen: (...a: Parameters<typeof mockListen>) => mockListen(...a),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
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

// Mock the perspective-container — BoardView reads `groupField` from it
// and does not need the real container's data fetches.
vi.mock("@/components/perspective-container", () => ({
  useActivePerspective: () => ({
    activePerspective: null,
    applySort: (entities: unknown[]) => entities,
    groupField: undefined,
  }),
}));

// ---------------------------------------------------------------------------
// Imports come after mocks.
// ---------------------------------------------------------------------------

import { BoardView } from "./board-view";
import { AppShell } from "./app-shell";
import { FocusLayer } from "./focus-layer";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { ActiveBoardPathProvider } from "@/lib/command-scope";
import { DragSessionProvider } from "@/lib/drag-session-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { UIStateProvider } from "@/lib/ui-state-context";
import { AppModeProvider } from "@/lib/app-mode-context";
import { UndoProvider } from "@/lib/undo-context";
import {
  asSegment,
  type FocusChangedPayload,
  type FullyQualifiedMoniker,
  type WindowLabel,
} from "@/types/spatial";
import { hasWebviewCommandHandler } from "@/lib/webview-command-bus";

// ---------------------------------------------------------------------------
// Test fixtures — three columns; the new task lands in the lowest-order
// column (`col-todo`) per the kernel's `resolve_focused_column` fallback the
// React-side focus dispatch mirrors.
// ---------------------------------------------------------------------------

/** The id the mocked `entity.add:task` dispatch reports back. */
const CREATED_TASK_ID = "t-new";

function makeColumn(id: string, name: string, order: number): Entity {
  return {
    entity_type: "column",
    id,
    moniker: `column:${id}`,
    fields: { name, order },
  };
}

function makeTask(id: string, columnId: string, ordinal: string): Entity {
  return {
    entity_type: "task",
    id,
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
  makeTask("t2", "col-doing", "a0"),
  makeTask("t3", "col-done", "a0"),
];

// ---------------------------------------------------------------------------
// Default invoke responses.
// ---------------------------------------------------------------------------

function makeDefaultInvokeImpl(keymapMode: "cua" | "vim") {
  return async function defaultInvokeImpl(
    cmd: string,
    args?: unknown,
  ): Promise<unknown> {
    // `board.newTask`'s keys reach the keymap layer only through the
    // `useCommandList` seam — answer `list command` with the shared mock
    // registry (the `board-commands` plugin mirror carries them).
    const listAnswer = answerListCommand(
      cmd,
      args,
      globalCommandsFromBindingTables(),
    );
    if (listAnswer) return listAnswer;
    if (cmd === "list_entity_types") return ["task", "column"];
    if (cmd === "get_entity_schema") {
      return {
        entity: { name: "task", entity_type: "task" },
        fields: [],
      };
    }
    if (cmd === "get_ui_state")
      return {
        palette_open: false,
        palette_mode: "command",
        keymap_mode: keymapMode,
        scope_chain: [],
        open_boards: [],
        windows: {},
        recent_boards: [],
      };
    if (cmd === "get_undo_state") return { can_undo: false, can_redo: false };
    if (cmd === "dispatch_command") {
      // The durable add reports the created task id back to the handler so
      // it can compose the card FQM and dispatch the focus jump.
      const a = args as { cmd?: string } | undefined;
      if (a?.cmd === "entity.add:task") return { id: CREATED_TASK_ID };
      return undefined;
    }
    return undefined;
  };
}

// ---------------------------------------------------------------------------
// Helpers — copied from `board-view.column-extremes.spatial.test.tsx` so this
// test stays self-contained.
// ---------------------------------------------------------------------------

async function flushSetup() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 50));
  });
}

async function fireFocusChanged({
  prev_fq = null,
  next_fq = null,
  next_segment = null,
}: {
  prev_fq?: FullyQualifiedMoniker | null;
  next_fq?: FullyQualifiedMoniker | null;
  next_segment?: string | null;
}) {
  const payload: FocusChangedPayload = {
    window_label: "main" as WindowLabel,
    prev_fq,
    next_fq,
    next_segment: next_segment as FocusChangedPayload["next_segment"],
  };
  const handlers = listeners.get("notifications/focus/changed") ?? [];
  await act(async () => {
    for (const handler of handlers) handler({ payload });
    await Promise.resolve();
  });
}

function renderBoardWithShell() {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <UIStateProvider>
            <AppModeProvider>
              <UndoProvider>
                <SchemaProvider>
                  <EntityStoreProvider entities={{}}>
                    <TooltipProvider>
                      <ActiveBoardPathProvider value="/test/board">
                        <DragSessionProvider>
                          <AppShell>
                            <BoardView board={board} tasks={tasks} />
                          </AppShell>
                        </DragSessionProvider>
                      </ActiveBoardPathProvider>
                    </TooltipProvider>
                  </EntityStoreProvider>
                </SchemaProvider>
              </UndoProvider>
            </AppModeProvider>
          </UIStateProvider>
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

function registerScopeArgs(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_scope")
    .map((c) => c[1] as Record<string, unknown>);
}

/** Every backend `dispatch_command` cmd id, in call order. */
function dispatchCmds(): string[] {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "dispatch_command")
    .map((c) => (c[1] as { cmd?: string } | undefined)?.cmd ?? "");
}

/** Every kernel focus-claim IPC (`spatial_focus` / focus `set focus`),
 * unwrapped to its args bag. */
function spatialFocusCalls(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter(
      (c) =>
        c[0] === "spatial_focus" ||
        (c[0] === "command_tool_call" &&
          (c[1] as Record<string, unknown>)?.tool === "focus" &&
          (c[1] as Record<string, unknown>)?.op === "set focus"),
    )
    .map((c) => {
      const outer = c[1] as Record<string, unknown>;
      return (outer?.params ?? outer) as Record<string, unknown>;
    });
}

// ---------------------------------------------------------------------------
// Tests — each block seeds focus on the middle column, fires the
// keystroke, and asserts the bus handler's orchestration: ONE durable
// `entity.add:task` dispatch, NO backend `board.newTask` dispatch, and a
// focus claim on the created card's FQM.
// ---------------------------------------------------------------------------

describe("BoardView — board.newTask runs on the webview bus and re-dispatches entity.add:task", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Drive a single keystroke and assert the `board.newTask` webview-bus
   * handler ran its orchestration.
   *
   * @param keymapMode `"vim"` for `o`, `"cua"` for `Mod+Enter`.
   * @param key        DOM event key.
   * @param eventInit  Extra `KeyboardEvent` init bits (e.g. `metaKey`).
   */
  async function assertNewTaskOrchestration({
    keymapMode,
    key,
    eventInit = {},
  }: {
    keymapMode: "vim" | "cua";
    key: string;
    eventInit?: KeyboardEventInit;
  }) {
    mockInvoke.mockImplementation(makeDefaultInvokeImpl(keymapMode));

    const { unmount } = renderBoardWithShell();
    await flushSetup();

    // The mounted board registers the `board.newTask` BEHAVIOR on the
    // webview command bus (Card B) — a registered handler is the signal the
    // id is "handled in webview", so `useDispatchCommand` runs it and skips
    // the backend. The definition itself lives in the `board-commands`
    // plugin, not in a React `CommandDef`.
    expect(
      hasWebviewCommandHandler("board.newTask"),
      "the board view must register the board.newTask webview-bus handler on mount",
    ).toBe(true);

    // Seed focus on the middle column so the focused scope chain (column →
    // board marker → …) is populated and the board-gated `board.newTask`
    // binding resolves.
    const middleColumn = registerScopeArgs().find(
      (a) => a.segment === "column:col-doing",
    );
    expect(middleColumn, "middle column zone must register").toBeTruthy();
    const middleColumnFq = middleColumn!.fq as FullyQualifiedMoniker;
    const boardZone = registerScopeArgs().find(
      (a) => a.segment === "board:board-1",
    );
    expect(boardZone, "board zone must register").toBeTruthy();
    const boardZoneFq = boardZone!.fq as string;
    await fireFocusChanged({
      next_fq: middleColumnFq,
      next_segment: asSegment("column:col-doing"),
    });

    mockInvoke.mockClear();

    await act(async () => {
      fireEvent.keyDown(document, { key, ...eventInit });
      // Let the handler's async add → focus chain settle.
      await new Promise((r) => setTimeout(r, 10));
    });

    // (1) The DURABLE add re-dispatches the backend-op `entity.add:task` —
    // exactly once — and the bus-intercepted `board.newTask` id itself never
    // reaches the backend.
    const dispatched = dispatchCmds();
    expect(
      dispatched.filter((cmd) => cmd === "entity.add:task"),
      `key "${key}" (${keymapMode}) must re-dispatch entity.add:task exactly once`,
    ).toHaveLength(1);
    expect(
      dispatched.filter((cmd) => cmd === "board.newTask"),
      "the bus handler intercepts board.newTask — it must not reach the backend",
    ).toHaveLength(0);

    // (2) The handler focuses the created card: the composed FQM is the
    // board zone + the fallback (lowest-order) column + the new task.
    const expectedCardFq = `${boardZoneFq}/column:col-todo/task:${CREATED_TASK_ID}`;
    const focusClaims = spatialFocusCalls().filter(
      (c) => c.fq === expectedCardFq,
    );
    expect(
      focusClaims.length,
      `the created card (${expectedCardFq}) must receive exactly one focus claim`,
    ).toBe(1);

    unmount();
  }

  it("cua 'Mod+Enter' adds a task via entity.add:task and focuses the new card", async () => {
    await assertNewTaskOrchestration({
      keymapMode: "cua",
      key: "Enter",
      eventInit: { metaKey: true },
    });
  });

  it("vim 'o' adds a task via entity.add:task and focuses the new card", async () => {
    await assertNewTaskOrchestration({
      keymapMode: "vim",
      key: "o",
    });
  });
});
