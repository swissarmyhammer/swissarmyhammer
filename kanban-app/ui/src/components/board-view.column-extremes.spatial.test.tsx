/**
 * Browser-mode test for `<BoardView>`'s column-extreme key bindings —
 * vim `0` / `$` and cua `Mod+Home` / `Mod+End`.
 *
 * Source of truth for the close-out of kanban task
 * `01KQJDKBQ2VNT3SE7AN3VM2KGZ` (audit: remove duplicate scope-local nav
 * commands that shadow global `nav.*` and route through the no-op
 * broadcast).
 *
 * **Pre-task behaviour**: `board.firstColumn` and `board.lastColumn`
 * (`makeNavBroadcastCommand` in `board-view.tsx`) bound vim `0` / `$`
 * and cua `Mod+Home` / `Mod+End` and threaded each press through
 * `broadcastRef.current("nav.first" | "nav.last")` — i.e. through
 * `FocusActions.broadcastNavCommand`, which was a no-op stub that
 * always returned `false`. Pressing those keys did nothing.
 *
 * **Post-task behaviour** (this test pins): the same key bindings now
 * call `spatialActions.navigate(focusedFq, "first" | "last")` directly,
 * which dispatches `spatial_navigate` to the Rust kernel exactly once
 * per press. The board's vim `0` / `$` and cua `Mod+Home` / `Mod+End`
 * keys are NOT in the global `NAV_COMMAND_SPEC` (`Home` / `End` are
 * cua there, vim has only `Shift+G` for last) — they fill a gap that
 * the global spec does not cover.
 *
 * Sister test to `board-view.spatial.test.tsx` — same harness, same
 * mock pattern, same browser project.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act, fireEvent } from "@testing-library/react";
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

function spatialNavigateCalls(): Array<{
  focusedFq: FullyQualifiedMoniker;
  direction: string;
}> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_navigate")
    .map(
      (c) => c[1] as { focusedFq: FullyQualifiedMoniker; direction: string },
    );
}

// ---------------------------------------------------------------------------
// Tests — one assertion block per key. Each block seeds focus on the
// middle column, fires the keystroke, and asserts a single
// `spatial_navigate` invocation with the expected direction.
// ---------------------------------------------------------------------------

describe("BoardView — column-extreme keys dispatch spatial_navigate", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Drive a single keystroke and assert the resulting `spatial_navigate`
   * call has the expected (focusedFq, direction) shape.
   *
   * Seeds focus on the middle column so "first" and "last" map to
   * distinct monikers. The middle column zone is registered by
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
   * @param direction  Expected `Direction` literal forwarded to
   *                   `spatial_navigate` — `"first"` or `"last"`.
   */
  async function assertSingleNavigate({
    keymapMode,
    key,
    eventInit = {},
    direction,
  }: {
    keymapMode: "vim" | "cua";
    key: string;
    eventInit?: KeyboardEventInit;
    direction: "first" | "last";
  }) {
    mockInvoke.mockImplementation(makeDefaultInvokeImpl(keymapMode));

    const { unmount } = renderBoardWithShell();
    await flushSetup();

    // Seed focus on the middle column so the kernel sees a non-null
    // `focusedFq`. `next_segment` is also seeded so the entity-focus
    // bridge mirrors the focused moniker into the entity-focus store —
    // without it the keybinding handler can't resolve scope-level
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

    const navCalls = spatialNavigateCalls();
    expect(
      navCalls.length,
      `key "${key}" (${keymapMode}) should dispatch spatial_navigate exactly once`,
    ).toBe(1);
    expect(navCalls[0].focusedFq).toBe(middleColumnFq);
    expect(navCalls[0].direction).toBe(direction);

    unmount();
  }

  it("vim '0' dispatches spatial_navigate first", async () => {
    await assertSingleNavigate({
      keymapMode: "vim",
      key: "0",
      direction: "first",
    });
  });

  it("vim '$' dispatches spatial_navigate last", async () => {
    // The keybinding pipeline normalises the canonical key for the
    // `$` glyph; it is produced by Shift+4 on a US layout but the
    // DOM `key` value is `"$"`. The CommandDef declares `vim: "$"`,
    // so the matching event is `key: "$", shiftKey: true`.
    await assertSingleNavigate({
      keymapMode: "vim",
      key: "$",
      eventInit: { shiftKey: true },
      direction: "last",
    });
  });

  it("cua 'Mod+Home' dispatches spatial_navigate first", async () => {
    // `Mod` is the keybinding registry's portable alias for `Meta` on
    // macOS / `Control` elsewhere. The browser test env runs on
    // Chromium where `Meta` is the canonical modifier — `metaKey: true`
    // matches `Mod`.
    await assertSingleNavigate({
      keymapMode: "cua",
      key: "Home",
      eventInit: { metaKey: true },
      direction: "first",
    });
  });

  it("cua 'Mod+End' dispatches spatial_navigate last", async () => {
    await assertSingleNavigate({
      keymapMode: "cua",
      key: "End",
      eventInit: { metaKey: true },
      direction: "last",
    });
  });
});
