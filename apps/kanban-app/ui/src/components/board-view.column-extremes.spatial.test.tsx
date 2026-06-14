/**
 * Browser-mode test for `<BoardView>`'s column-extreme key bindings —
 * vim `0` / `$` and cua `Mod+Home` / `Mod+End`.
 *
 * **Card F behaviour** (this test pins): `board.firstColumn` /
 * `board.lastColumn` are DEFINED by the `board-commands` builtin plugin
 * (`builtin/plugins/board-commands/index.ts`), scope-gated to the
 * `ui:board` marker the board view mounts. Their execution is a real
 * BACKEND route — the plugin's host execute drives the focus kernel's
 * `navigate focus` op (first / last) — so a key press resolves the
 * binding through the registry chain walk and dispatches the command id
 * to the backend (`invoke("dispatch_command", { cmd: "board.*" })`)
 * exactly once, with NO client-side `spatial_navigate` kernel IPC (the
 * retired React `makeNavCommand` defs called
 * `spatialActions.navigate(focusedFq, direction)` in the webview).
 *
 * The board's vim `0` / `$` and cua `Mod+Home` / `Mod+End` keys are NOT
 * among the `nav-commands` plugin's `nav.first` / `nav.last` keys
 * (`Home` / `End` are cua there, vim has only `Shift+G` for last) —
 * they fill a gap the plugin does not cover, gated to the board zone.
 *
 * Sister test to `board-view.spatial.test.tsx` — same harness, same
 * mock pattern, same browser project.
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

type ListenCallback = (event: { payload: unknown }) => void;

const { mockInvoke, mockListen, listeners } = vi.hoisted(() => {
  const listeners = new Map<string, ListenCallback[]>();
  const mockInvoke = vi.fn(
    async (_cmd: string, _args?: unknown): Promise<unknown> => undefined,
  );
  const mockListen = vi.fn(
    (eventName: string, cb: ListenCallback): Promise<() => void> => {
      const cbs = listeners.get(eventName) ?? [];
      cbs.push(cb);
      listeners.set(eventName, cbs);
      return Promise.resolve(() => {
        const arr = listeners.get(eventName);
        if (arr) {
          const idx = arr.indexOf(cb);
          if (idx >= 0) arr.splice(idx, 1);
        }
      });
    },
  );
  return { mockInvoke, mockListen, listeners };
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

// ---------------------------------------------------------------------------
// Test fixtures — three columns so "first" and "last" land on distinct
// monikers and the seeded middle column is unambiguous.
// ---------------------------------------------------------------------------

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
// Default invoke responses — override with `mockInvoke.mockImplementation`
// inside individual tests when a different keymap mode is needed.
// ---------------------------------------------------------------------------

function makeDefaultInvokeImpl(keymapMode: "cua" | "vim" | "emacs") {
  return async function defaultInvokeImpl(
    cmd: string,
    _args?: unknown,
  ): Promise<unknown> {
    // The board commands are DEFINED by the `board-commands` builtin plugin
    // (scope ["ui:board"]) — their keys reach the keymap layer only through
    // the `useCommandList` seam, so answer `list command` with the shared
    // mock registry.
    const listAnswer = answerListCommand(
      cmd,
      _args,
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
    if (cmd === "dispatch_command") return undefined;
    return undefined;
  };
}

// ---------------------------------------------------------------------------
// Helpers — copied from `board-view.spatial.test.tsx` so this test stays
// self-contained.
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
  const handlers = listeners.get("focus-changed") ?? [];
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

/** Every CLIENT-SIDE kernel-navigate IPC — the retired React defs' wire
 * shape. Post-Card-F the webview must never fire one of these for a
 * column-extreme key: the focus op runs host-side in the plugin. */
function clientSpatialNavigateCalls(): Array<unknown> {
  return mockInvoke.mock.calls.filter(
    (c) =>
      c[0] === "spatial_navigate" ||
      (c[0] === "command_tool_call" &&
        (c[1] as Record<string, unknown>)?.tool === "focus" &&
        (c[1] as Record<string, unknown>)?.op === "navigate focus"),
  );
}

/** Every backend `dispatch_command` whose cmd is a `board.*` id, in order —
 * the post-Card-F contract: the webview routes the command id to the
 * backend, where the `board-commands` plugin drives the focus kernel. */
function boardDispatchCmds(): string[] {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "dispatch_command")
    .map((c) => (c[1] as { cmd?: string } | undefined)?.cmd ?? "")
    .filter((cmd) => cmd.startsWith("board."));
}

// ---------------------------------------------------------------------------
// Tests — one assertion block per key. Each block seeds focus on the
// middle column, fires the keystroke, and asserts a single backend
// dispatch of the expected `board.*` command id.
// ---------------------------------------------------------------------------

describe("BoardView — column-extreme keys dispatch the plugin board.* commands to the backend", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Drive a single keystroke and assert it resolves to exactly one backend
   * dispatch of the expected `board.*` command id — with no client-side
   * kernel-navigate IPC (the plugin's host execute owns the focus op).
   *
   * Seeds focus on the middle column so the binding resolves through the
   * focused scope chain (the `ui:board` marker the board mounts gates the
   * plugin commands' keys). The middle column zone is registered by
   * `<ColumnView>` under the board zone — we look it up by segment from
   * the recorded `spatial_register_scope` IPCs.
   *
   * @param keymapMode `"vim"` for `0` / `$`, `"cua"` for `Mod+Home` /
   *                   `Mod+End`. Drives `get_ui_state`'s `keymap_mode`
   *                   so the keybinding handler resolves the right
   *                   table.
   * @param key        DOM event key (vim `0` / `$` are literal digits and
   *                   shift+4 respectively; cua `Mod+Home` is `Home` with
   *                   `metaKey`).
   * @param eventInit  Extra `KeyboardEvent` init bits (e.g. `metaKey`).
   * @param commandId  Expected `board.*` command id dispatched to the
   *                   backend — `"board.firstColumn"` or
   *                   `"board.lastColumn"`.
   */
  async function assertSingleBoardDispatch({
    keymapMode,
    key,
    eventInit = {},
    commandId,
  }: {
    keymapMode: "vim" | "cua";
    key: string;
    eventInit?: KeyboardEventInit;
    commandId: "board.firstColumn" | "board.lastColumn";
  }) {
    mockInvoke.mockImplementation(makeDefaultInvokeImpl(keymapMode));

    const { unmount } = renderBoardWithShell();
    await flushSetup();

    // Seed focus on the middle column so the focused scope chain (column →
    // board marker → …) is populated. `next_segment` is also seeded so the
    // entity-focus bridge mirrors the focused moniker into the entity-focus
    // store — without it the keybinding handler can't resolve scope-level
    // bindings.
    const middleColumn = registerScopeArgs().find(
      (a) => a.segment === "column:col-doing",
    );
    expect(middleColumn, "middle column zone must register").toBeTruthy();
    const middleColumnFq = middleColumn!.fq as FullyQualifiedMoniker;
    await fireFocusChanged({
      next_fq: middleColumnFq,
      next_segment: asSegment("column:col-doing"),
    });

    mockInvoke.mockClear();

    await act(async () => {
      fireEvent.keyDown(document, { key, ...eventInit });
      await Promise.resolve();
    });

    const dispatched = boardDispatchCmds();
    expect(
      dispatched,
      `key "${key}" (${keymapMode}) should dispatch ${commandId} to the backend exactly once`,
    ).toEqual([commandId]);
    // The focus op runs HOST-SIDE in the board-commands plugin — the webview
    // must not fire its own kernel-navigate IPC (the retired React def did).
    expect(
      clientSpatialNavigateCalls(),
      "no client-side spatial_navigate IPC may fire",
    ).toEqual([]);

    unmount();
  }

  it("vim '0' dispatches board.firstColumn to the backend", async () => {
    await assertSingleBoardDispatch({
      keymapMode: "vim",
      key: "0",
      commandId: "board.firstColumn",
    });
  });

  it("vim '$' dispatches board.lastColumn to the backend", async () => {
    // The keybinding pipeline normalises the canonical key for the
    // `$` glyph; it is produced by Shift+4 on a US layout but the
    // DOM `key` value is `"$"`. The plugin def declares `vim: "$"`,
    // so the matching event is `key: "$", shiftKey: true`.
    await assertSingleBoardDispatch({
      keymapMode: "vim",
      key: "$",
      eventInit: { shiftKey: true },
      commandId: "board.lastColumn",
    });
  });

  it("cua 'Mod+Home' dispatches board.firstColumn to the backend", async () => {
    // `Mod` is the keybinding registry's portable alias for `Meta` on
    // macOS / `Control` elsewhere. The browser test env runs on
    // Chromium where `Meta` is the canonical modifier — `metaKey: true`
    // matches `Mod`.
    await assertSingleBoardDispatch({
      keymapMode: "cua",
      key: "Home",
      eventInit: { metaKey: true },
      commandId: "board.firstColumn",
    });
  });

  it("cua 'Mod+End' dispatches board.lastColumn to the backend", async () => {
    await assertSingleBoardDispatch({
      keymapMode: "cua",
      key: "End",
      eventInit: { metaKey: true },
      commandId: "board.lastColumn",
    });
  });
});
