/**
 * Browser-mode tests pinning the "Enter drills in, not inspect" contract on
 * the board surface.
 *
 * Source of truth for card `01KQ9X3A9NMRYK50GWP4S4ZMJ4`. Before this card
 * the BoardView's `<CommandScopeProvider>` registered a `board.inspect`
 * command keyed to vim Enter. Because that scope is closer than the root
 * scope in `extractScopeBindings`, vim Enter on every focused entity
 * inside the board shadowed the global `nav.drillIn` and dispatched
 * `ui.inspect` instead of drilling in. After the fix:
 *
 *   - vim Enter on a focused card no longer dispatches `ui.inspect`.
 *     The card is a leaf today (no zone children), so `nav.drillIn`'s
 *     execute closure invokes `spatial_drill_in` and the kernel
 *     returns `null` — Enter is a no-op. The test pins this by
 *     asserting zero `ui.inspect` invocations on the dispatch trace.
 *   - cua Enter has never been bound to inspect; the same regression
 *     guard applies in cua mode for completeness.
 *   - cua Space still dispatches `ui.inspect` against the focused
 *     entity — the per-`<Inspectable>` scope-level command kept the
 *     Space binding (see card 01KQ9XJ4XGKVW24EZSQCA6K3E2).
 *   - vim Enter on a focused column drills into the column's first
 *     card via `spatial_drill_in` → `column:<id>` resolves to the
 *     first task moniker.
 *   - vim Enter on a focused column with a remembered `last_focused`
 *     drills back into that remembered card, not the structural
 *     first child.
 *
 * Mock pattern matches `board-view.spatial.test.tsx` so the two files
 * stay in sync as the BoardView spatial contract evolves.
 *
 * Runs under `kanban-app/ui/vite.config.ts`'s browser project (real
 * Chromium via Playwright) — every `*.test.tsx` outside
 * `*.node.test.tsx` lands here.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act, fireEvent, waitFor } from "@testing-library/react";
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
  type WindowLabel
} from "@/types/spatial";

// ---------------------------------------------------------------------------
// Test fixtures
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
  makeTask("t2", "col-todo", "a1"),
  makeTask("t3", "col-doing", "a0"),
];

// ---------------------------------------------------------------------------
// Mutable keymap so tests can switch cua/vim per-case without remounting.
// ---------------------------------------------------------------------------

let mockKeymapMode: "cua" | "vim" | "emacs" = "cua";

/**
 * Tracks the moniker → FullyQualifiedMoniker mapping so `spatial_focus_by_moniker`
 * can synthesize the kernel's `focus-changed` emit. Card
 * `01KQD0WK54G0FRD7SZVZASA9ST` made the entity-focus store a pure
 * projection of kernel events; tests that mock `invoke` without a
 * kernel simulator need this minimal stub so `setFocus(moniker)`
 * still flows through the spatial-focus bridge into the React store.
 */
const monikerToKey = new Map<string, string>();
const currentFocusKey: { key: string | null } = { key: null };

// ---------------------------------------------------------------------------
// Default invoke responses — the handful of IPCs the AppShell + BoardView
// providers hit on mount. Kept in one place so beforeEach restores them
// cleanly after each test's mockClear / mockReset.
// ---------------------------------------------------------------------------

async function defaultInvokeImpl(
  cmd: string,
  args?: unknown,
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
      keymap_mode: mockKeymapMode,
      scope_chain: [],
      open_boards: [],
      windows: {},
      recent_boards: [],
    };
  if (cmd === "get_undo_state") return { can_undo: false, can_redo: false };
  if (cmd === "dispatch_command") return undefined;
  // The spatial-nav register/unregister/focus/navigate calls all return
  // void — undefined is the safe default. drill_in defaults to null
  // (no resolvable child) which is the expected leaf-card path.
  if (cmd === "spatial_drill_in") return null;
  if (cmd === "spatial_register_scope" || cmd === "spatial_register_scope") {
    const a = (args ?? {}) as { fq?: string; segment?: string };
    if (a.fq && a.segment) monikerToKey.set(a.segment, a.fq);
    return undefined;
  }
  if (cmd === "spatial_unregister_scope") {
    const a = (args ?? {}) as { fq?: string };
    if (a.fq) {
      for (const [m, k] of monikerToKey.entries()) {
        if (k === a.fq) {
          monikerToKey.delete(m);
          break;
        }
      }
    }
    return undefined;
  }
  if (cmd === "spatial_focus") {
    // Queued via `queueMicrotask` to match the kernel simulator and
    // real Tauri events — emitting synchronously would hide
    // regressions where `setFocus` writes the store synchronously.
    const a = (args ?? {}) as { fq?: string };
    const fq = a.fq ?? null;
    let moniker: string | null = null;
    for (const [s, k] of monikerToKey.entries()) {
      if (k === fq) { moniker = s; break; }
    }
    
    if (fq) {
      const prev = currentFocusKey.key;
      currentFocusKey.key = fq;
      queueMicrotask(() => {
        const handlers = listeners.get("focus-changed") ?? [];
        for (const handler of handlers) {
          handler({
            payload: {
              window_label: "main",
              prev_fq: prev,
              next_fq: fq,
              next_segment: moniker,
            },
          });
        }
      });
    }
    return undefined;
  }
  if (cmd === "spatial_clear_focus") {
    const prev = currentFocusKey.key;
    if (prev === null) return undefined;
    currentFocusKey.key = null;
    queueMicrotask(() => {
      const handlers = listeners.get("focus-changed") ?? [];
      for (const handler of handlers) {
        handler({
          payload: {
            window_label: "main",
            prev_fq: prev,
            next_fq: null,
            next_segment: null,
          },
        });
      }
    });
    return undefined;
  }
  return undefined;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Wait for register effects scheduled in `useEffect` to flush. */
async function flushSetup() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 50));
  });
}

/**
 * Drive a `focus-changed` event into the React tree as if the Rust kernel
 * had emitted one for the active window.
 *
 * The `next_segment` argument is REQUIRED for keystroke tests: the
 * spatial → entity bridge in `<EntityFocusProvider>` calls
 * `actions.setFocus(payload.next_segment)` on every focus-changed
 * event. The entity-focus store's `focusedScope` is what AppShell's
 * `<KeybindingHandler>` walks via `extractScopeBindings` to resolve
 * scope-level command keys.
 */
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

/**
 * Render `<BoardView>` inside the production-shaped spatial-nav stack,
 * wrapped by `<AppShell>` so the global keybinding pipeline is live.
 *
 * The AppShell mounts `<KeybindingHandler>` which attaches a `keydown`
 * listener on `document` and dispatches the focused scope's commands.
 * That is what turns Enter into the `nav.drillIn` execute closure
 * invocation — without the AppShell those keystrokes would land in the
 * void.
 */
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

/** Pull every `spatial_register_scope` invocation argument bag. */
function registerScopeArgs(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_scope")
    .map((c) => c[1] as Record<string, unknown>);
}

/** Pull every `spatial_drill_in` call's args, in order. */
function spatialDrillInCalls(): Array<{ fq: FullyQualifiedMoniker }> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_drill_in")
    .map((c) => c[1] as { fq: FullyQualifiedMoniker });
}

/** Filter `dispatch_command` calls down to those for `ui.inspect`. */
function inspectDispatches(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "dispatch_command")
    .map((c) => c[1] as Record<string, unknown>)
    .filter((p) => p.cmd === "ui.inspect");
}

/**
 * Find the registered FullyQualifiedMoniker for a given moniker. The
 * board zone, columns, and cards all register via
 * `spatial_register_scope` (post-card-`01KQJDYJ4SDKK2G8FTAQ348ZHG`);
 * the scope-fallback path is kept for any leaf segment a future test
 * driver might emit.
 */
function keyForMoniker(moniker: string): FullyQualifiedMoniker | undefined {
  const zone = registerScopeArgs().find((a) => a.segment === moniker);
  if (zone) return zone.fq as FullyQualifiedMoniker;
  const scope = registerScopeArgs().find((a) => a.segment === moniker);
  return scope?.fq as FullyQualifiedMoniker | undefined;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("BoardView — Enter drills in, not inspect", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    monikerToKey.clear();
    currentFocusKey.key = null;
    mockKeymapMode = "cua";
    mockInvoke.mockImplementation(defaultInvokeImpl);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // -------------------------------------------------------------------------
  // #1: vim Enter on a focused card does NOT dispatch ui.inspect
  // -------------------------------------------------------------------------

  it("enter_on_focused_card_does_not_dispatch_inspect_in_vim", async () => {
    mockKeymapMode = "vim";
    const { unmount } = renderBoardWithShell();
    await flushSetup();

    // Cards register as zones — find the first task's zone key.
    const cardKey = keyForMoniker("task:t1");
    expect(
      cardKey,
      "the first card must register a spatial zone",
    ).toBeTruthy();

    // Drive a focus-changed event so the entity-focus store reflects
    // the card moniker. `extractScopeBindings` reads the focused
    // scope chain on the next keydown.
    await fireFocusChanged({
      next_fq: cardKey!,
      next_segment: asSegment("task:t1"),
    });
    await flushSetup();

    // Reset the dispatch / drill spies so we measure only the keystroke.
    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });
    await flushSetup();

    // The focused-card path: vim Enter must NOT dispatch ui.inspect.
    expect(
      inspectDispatches().length,
      "vim Enter on a focused card must dispatch zero ui.inspect calls",
    ).toBe(0);

    unmount();
  });

  // -------------------------------------------------------------------------
  // #2: cua Enter on a focused card does NOT dispatch ui.inspect (regression)
  // -------------------------------------------------------------------------

  it("enter_on_focused_card_does_not_dispatch_inspect_in_cua", async () => {
    mockKeymapMode = "cua";
    const { unmount } = renderBoardWithShell();
    await flushSetup();

    const cardKey = keyForMoniker("task:t1");
    expect(cardKey).toBeTruthy();

    await fireFocusChanged({
      next_fq: cardKey!,
      next_segment: asSegment("task:t1"),
    });
    await flushSetup();

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });
    await flushSetup();

    // Regression guard — cua Enter has never been bound to inspect.
    expect(
      inspectDispatches().length,
      "cua Enter on a focused card must dispatch zero ui.inspect calls",
    ).toBe(0);

    unmount();
  });

  // -------------------------------------------------------------------------
  // #3: cua Space on a focused card still dispatches ui.inspect
  // -------------------------------------------------------------------------

  it("space_on_focused_card_still_dispatches_inspect_in_cua", async () => {
    mockKeymapMode = "cua";
    const { unmount } = renderBoardWithShell();
    await flushSetup();

    const cardKey = keyForMoniker("task:t1");
    expect(cardKey).toBeTruthy();

    await fireFocusChanged({
      next_fq: cardKey!,
      next_segment: asSegment("task:t1"),
    });
    await flushSetup();

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    // Fire Space at the document level — the `<Inspectable>` wrapper's
    // scope-level `entity.inspect` command is keyed `cua: "Space"`,
    // closer in the scope chain than the global root, and resolves
    // through `extractScopeBindings`.
    await act(async () => {
      fireEvent.keyDown(document, { key: " ", code: "Space" });
      await Promise.resolve();
    });
    await flushSetup();

    const dispatches = inspectDispatches();
    expect(
      dispatches.length,
      "cua Space on a focused card must dispatch ui.inspect exactly once",
    ).toBe(1);
    expect(
      dispatches[0].target,
      "ui.inspect from a focused card must carry that card's moniker",
    ).toBe("task:t1");

    unmount();
  });

  // -------------------------------------------------------------------------
  // #4: vim Enter on a focused column drills into the column's first card
  // -------------------------------------------------------------------------

  it("enter_on_focused_column_drills_into_first_card", async () => {
    mockKeymapMode = "vim";
    const { unmount } = renderBoardWithShell();
    await flushSetup();

    const columnKey = keyForMoniker("column:col-todo");
    expect(
      columnKey,
      "the col-todo column must register a spatial zone",
    ).toBeTruthy();

    // Seed focus to the column zone. The bridge mirrors next_segment
    // into the entity-focus store so `extractScopeBindings` walks the
    // column's scope chain on the next Enter keydown.
    await fireFocusChanged({
      next_fq: columnKey!,
      next_segment: asSegment("column:col-todo"),
    });
    await flushSetup();

    mockInvoke.mockClear();

    // Have the kernel resolve drill-in for the column to the first
    // card moniker. The drill closure dispatches `setFocus(moniker)`,
    // which fans out to a `dispatch_command(ui.setFocus, …)` IPC.
    mockInvoke.mockImplementation(async (cmd, args) => {
      if (cmd === "spatial_drill_in") {
        return "task:t1";
      }
      return defaultInvokeImpl(cmd, args);
    });

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });
    await flushSetup();

    // Enter dispatched `nav.drillIn` for the focused column key.
    const drillCalls = spatialDrillInCalls();
    expect(
      drillCalls.length,
      "vim Enter on a focused column must dispatch spatial_drill_in exactly once",
    ).toBe(1);
    expect(drillCalls[0].fq).toBe(columnKey);

    // The closure's success branch forwards the kernel-returned
    // moniker to `FocusActions.setFocus`, which under the
    // `SpatialFocusProvider` (production) path invokes
    // `spatial_focus` with that moniker. The kernel then echoes a
    // `focus-changed` event that the bridge mirrors into the entity
    // focus store. Confirm that the `spatial_focus` fanout fires and
    // carries the resolved card moniker.
    const focusCall = mockInvoke.mock.calls.find(
      (c) => c[0] === "spatial_focus",
    );
    expect(focusCall).toBeTruthy();
    const focusArgs = focusCall![1] as { fq?: string };
    expect(
      focusArgs.fq,
      "drill-in's setFocus must invoke spatial_focus with the resolved child moniker",
    ).toBe("task:t1");

    unmount();
  });

  // -------------------------------------------------------------------------
  // #5: vim Enter on a focused column with remembered focus drills into the
  //     remembered card (kernel-resolved last_focused).
  // -------------------------------------------------------------------------

  it("enter_on_focused_column_with_remembered_focus_drills_into_remembered_card", async () => {
    // The kernel owns last_focused memory — it returns whichever
    // moniker matches the column's most recently focused descendant
    // (or the structural first child when nothing has been focused
    // yet). The React side observes this contract by trusting the
    // moniker returned from `spatial_drill_in`. This test pins the
    // contract by stubbing the kernel to return `task:t2` (the
    // non-first card in col-todo) and asserting the React fanout
    // mirrors it via setFocus.
    mockKeymapMode = "vim";
    const { unmount } = renderBoardWithShell();
    await flushSetup();

    const columnKey = keyForMoniker("column:col-todo");
    expect(columnKey).toBeTruthy();
    // Capture the t2 card's FullyQualifiedMoniker before clearing the mock call
    // log — its registration happened during mount.
    const t2Key = keyForMoniker("task:t2");
    expect(
      t2Key,
      "the t2 card must register a spatial scope as a leaf during mount",
    ).toBeTruthy();

    await fireFocusChanged({
      next_fq: columnKey!,
      next_segment: asSegment("column:col-todo"),
    });
    await flushSetup();

    mockInvoke.mockClear();

    // Pretend the kernel previously remembered task:t2 as the
    // column's last focused descendant.
    mockInvoke.mockImplementation(async (cmd, args) => {
      if (cmd === "spatial_drill_in") {
        return "task:t2";
      }
      return defaultInvokeImpl(cmd, args);
    });

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });
    await flushSetup();

    const drillCalls = spatialDrillInCalls();
    expect(drillCalls.length).toBe(1);
    expect(drillCalls[0].fq).toBe(columnKey);

    const focusCall = mockInvoke.mock.calls.find(
      (c) => c[0] === "spatial_focus",
    );
    expect(focusCall).toBeTruthy();
    const focusArgs = focusCall![1] as { fq?: string };
    expect(
      focusArgs.fq,
      "drill-in must follow the kernel-returned remembered moniker",
    ).toBe("task:t2");

    // Belt-and-suspenders: a synthetic focus-changed event for the
    // remembered card flips its data-focused on the DOM side.
    await fireFocusChanged({
      prev_fq: columnKey!,
      next_fq: t2Key!,
      next_segment: asSegment("task:t2"),
    });
    await waitFor(() => {
      const t2Node = document.querySelector(
        "[data-segment='task:t2']",
      ) as HTMLElement | null;
      expect(t2Node).not.toBeNull();
      expect(t2Node!.getAttribute("data-focused")).not.toBeNull();
    });

    unmount();
  });
});
