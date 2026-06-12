/**
 * Browser-mode tests pinning the "Enter drills in, not inspect" contract on
 * the board surface.
 *
 * Covers:
 *   - vim Enter on a focused card does not dispatch `app.inspect`.
 *   - cua Enter on a focused card does not dispatch `app.inspect`.
 *   - cua Space on a focused card still dispatches `app.inspect` against
 *     the focused entity via the per-`<Inspectable>` scope-level command.
 *   - vim Enter on a focused column routes the `nav.drillIn` command id
 *     to the backend (`dispatch_command`); the kernel resolves the
 *     column's first card host-side and the webview mirrors the
 *     resulting `focus-changed` emission.
 *   - vim Enter on a focused column with a remembered `last_focused`
 *     follows the kernel's remembered-card `focus-changed` the same way.
 *   - The drill wire carries NO webview-built geometry snapshot and no
 *     pre-resolved fq — drill executes in the `nav-commands` builtin
 *     plugin and the kernel pulls live geometry on demand. The webview
 *     issues no client-side `spatial_drill_in` / `spatial_drill_out`
 *     IPC and no drill-driven `spatial_focus` fan-out.
 *
 * Runs under the browser project (real Chromium via Playwright) — every
 * `*.test.tsx` outside `*.node.test.tsx` lands here.
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
import { commandToolCall, navDispatchCmds } from "@/test/mock-command-list";
import { wrapMcpDispatch } from "@/test/mcp-invoke-translator";
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
  // Post-Stage-3, focus / entity operations route through the MCP
  // envelope `invoke("command_tool_call", { tool, op, params })`. Detect
  // a focus-tool envelope and re-enter `defaultInvokeImpl` with the
  // legacy `(cmd, args)` shape so the rest of this dispatcher (which
  // pre-dates the migration) matches without changes. The
  // `mock-command-list` `commandToolCall` is reserved for the
  // commands-tool ops (`list command`, `available command`).
  if (cmd === "command_tool_call") {
    const env = args as
      | { tool?: string; op?: string; params?: Record<string, unknown> }
      | undefined;
    if (env?.tool === "focus" || env?.tool === "entity") {
      const wrapped = wrapMcpDispatch(
        // Stub a `mock.calls` array so the translator's call-replacement
        // logic has a sink — we don't surface translated entries here
        // because this codepath is invoked from a custom dispatcher,
        // not the spy's own `mockImplementation`.
        { mock: { calls: [] } },
        (legacyCmd: string, legacyArgs?: unknown) =>
          defaultInvokeImpl(legacyCmd, legacyArgs),
      );
      return wrapped(cmd, args);
    }
    return commandToolCall(args);
  }
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
  // The spatial-nav register/unregister/focus calls all return void —
  // undefined is the safe default. Drill has no client-side IPC at all:
  // it executes host-side in the `nav-commands` builtin plugin.
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
      if (k === fq) {
        moniker = s;
        break;
      }
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
 * `<KeybindingHandler>` walks via `extractChainBindings` to resolve
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

/** Collect every client-side `drill_in layer` IPC, in order. Host-driven
 * drill means this must stay EMPTY for keyboard drill-in — the kernel
 * executes the drill in the `nav-commands` builtin plugin. The legacy
 * bare `spatial_drill_in` cmd is matched too as a no-legacy guard. */
function spatialDrillInCalls(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter(
      (c) =>
        c[0] === "spatial_drill_in" ||
        (c[0] === "command_tool_call" &&
          (c[1] as any)?.tool === "focus" &&
          (c[1] as any)?.op === "drill_in layer"),
    )
    .map((c) => {
      const outer = c[1] as Record<string, unknown>;
      return (outer?.params ?? outer) as Record<string, unknown>;
    });
}

/** Collect every client-side `drill_out layer` IPC, in order. Host-driven
 * drill means this must stay EMPTY for keyboard drill-out — see
 * {@link spatialDrillInCalls}. */
function spatialDrillOutCalls(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter(
      (c) =>
        c[0] === "spatial_drill_out" ||
        (c[0] === "command_tool_call" &&
          (c[1] as any)?.tool === "focus" &&
          (c[1] as any)?.op === "drill_out layer"),
    )
    .map((c) => {
      const outer = c[1] as Record<string, unknown>;
      return (outer?.params ?? outer) as Record<string, unknown>;
    });
}

/** Collect every client-side `set focus` IPC, in order. The drill flow
 * must NOT fan out a webview-side `spatial_focus` — the kernel commits
 * focus host-side and the webview only mirrors the resulting
 * `focus-changed` emission. */
function spatialFocusCalls(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter(
      (c) =>
        c[0] === "spatial_focus" ||
        (c[0] === "command_tool_call" &&
          (c[1] as any)?.tool === "focus" &&
          (c[1] as any)?.op === "set focus"),
    )
    .map((c) => {
      const outer = c[1] as Record<string, unknown>;
      return (outer?.params ?? outer) as Record<string, unknown>;
    });
}

/** Filter `dispatch_command` calls down to those for the given command id. */
function dispatchPayloads(cmdId: string): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "dispatch_command")
    .map((c) => c[1] as Record<string, unknown>)
    .filter((p) => p.cmd === cmdId);
}

/** Filter `dispatch_command` calls down to those for `app.inspect`. */
function inspectDispatches(): Array<Record<string, unknown>> {
  return dispatchPayloads("app.inspect");
}

/**
 * Filter `dispatch_command` calls down to those for the plugin-owned
 * `entity.inspect` (Card G). Space routes this id to the BACKEND with the
 * focused scope chain; the plugin resolves the target server-side.
 */
function entityInspectDispatches(): Array<Record<string, unknown>> {
  return dispatchPayloads("entity.inspect");
}

/**
 * Find the registered FullyQualifiedMoniker for a given segment moniker by
 * scanning `spatial_register_scope` calls.
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
  // #1: vim Enter on a focused card does NOT dispatch app.inspect
  // -------------------------------------------------------------------------

  it("enter_on_focused_card_does_not_dispatch_inspect_in_vim", async () => {
    mockKeymapMode = "vim";
    const { unmount } = renderBoardWithShell();
    await flushSetup();

    // Cards register as zones — find the first task's zone key.
    const cardKey = keyForMoniker("task:t1");
    expect(cardKey, "the first card must register a spatial zone").toBeTruthy();

    // Drive a focus-changed event so the entity-focus store reflects
    // the card moniker. `extractChainBindings` reads the focused
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

    // The focused-card path: vim Enter must NOT dispatch app.inspect.
    expect(
      inspectDispatches().length,
      "vim Enter on a focused card must dispatch zero app.inspect calls",
    ).toBe(0);

    unmount();
  });

  // -------------------------------------------------------------------------
  // #2: cua Enter on a focused card does NOT dispatch app.inspect (regression)
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
      "cua Enter on a focused card must dispatch zero app.inspect calls",
    ).toBe(0);

    unmount();
  });

  // -------------------------------------------------------------------------
  // #3: cua Space on a focused card still dispatches app.inspect
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

    // Fire Space at the document level — the GLOBAL `entity.inspect`
    // binding (plugin-owned, Card G) resolves and routes the dispatch to
    // the BACKEND with the focused scope chain; the plugin resolves the
    // card's moniker server-side from the chain's leaf-first head.
    await act(async () => {
      fireEvent.keyDown(document, { key: " ", code: "Space" });
      await Promise.resolve();
    });
    await flushSetup();

    const dispatches = entityInspectDispatches();
    expect(
      dispatches.length,
      "cua Space on a focused card must dispatch entity.inspect exactly once",
    ).toBe(1);
    expect(
      (dispatches[0].scopeChain as string[] | undefined)?.[0],
      "the dispatched chain's head must be the focused card's moniker",
    ).toBe("task:t1");
    expect(
      inspectDispatches().length,
      "Space must not synthesize a webview-side app.inspect — the backend owns the inspect",
    ).toBe(0);

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
    // Capture the first card's FQM before clearing the mock call log —
    // its registration happened during mount.
    const t1Key = keyForMoniker("task:t1");
    expect(
      t1Key,
      "the t1 card must register a spatial scope during mount",
    ).toBeTruthy();

    // Seed focus to the column zone. The bridge mirrors next_segment
    // into the entity-focus store so `extractChainBindings` walks the
    // column's scope chain on the next Enter keydown.
    await fireFocusChanged({
      next_fq: columnKey!,
      next_segment: asSegment("column:col-todo"),
    });
    await flushSetup();

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });
    await flushSetup();

    // Enter routes the `nav.drillIn` plugin command id to the backend —
    // drill executes host-side in the `nav-commands` builtin plugin,
    // which resolves the focused scope and pulls geometry itself. The
    // webview sends no fq, no snapshot, and no client-side drill IPC.
    expect(
      navDispatchCmds(mockInvoke),
      "vim Enter on a focused column must dispatch nav.drillIn to the backend exactly once",
    ).toEqual(["nav.drillIn"]);
    expect(
      (dispatchPayloads("nav.drillIn")[0].scopeChain as string[])[0],
      "the dispatched chain's head must be the focused column's moniker",
    ).toBe("column:col-todo");
    expect(
      spatialDrillInCalls(),
      "no legacy client-side spatial_drill_in IPC may leave the webview",
    ).toHaveLength(0);
    expect(
      spatialFocusCalls(),
      "no webview-side spatial_focus fan-out — the kernel commits focus host-side",
    ).toHaveLength(0);

    // The kernel resolves the column's first card, commits focus to it,
    // and emits `focus-changed`; mimic that emission and confirm the
    // entity-focus bridge mirrors it — the card flips its data-focused.
    await fireFocusChanged({
      prev_fq: columnKey!,
      next_fq: t1Key!,
      next_segment: asSegment("task:t1"),
    });
    await waitFor(() => {
      const t1Node = document.querySelector(
        "[data-segment='task:t1']",
      ) as HTMLElement | null;
      expect(t1Node).not.toBeNull();
      expect(t1Node!.getAttribute("data-focused")).not.toBeNull();
    });

    unmount();
  });

  // -------------------------------------------------------------------------
  // #5: vim Enter on a focused column with remembered focus drills into the
  //     remembered card (kernel-resolved last_focused).
  // -------------------------------------------------------------------------

  it("enter_on_focused_column_with_remembered_focus_drills_into_remembered_card", async () => {
    // The kernel owns last_focused memory — it commits focus to
    // whichever moniker matches the column's most recently focused
    // descendant (or the structural first child when nothing has been
    // focused yet). That resolution executes host-side in the
    // `nav-commands` builtin plugin (pinned by the kernel-side e2e,
    // `builtin_nav_commands_e2e.rs`). The webview's contract is to
    // route `nav.drillIn` to the backend and mirror whatever
    // `focus-changed` the kernel emits — here mimicked for `task:t2`,
    // the NON-first card in col-todo.
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
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });
    await flushSetup();

    // Enter routes the `nav.drillIn` id to the backend exactly once —
    // no client-side drill IPC, no webview-side focus fan-out.
    expect(
      navDispatchCmds(mockInvoke),
      "vim Enter on a focused column must dispatch nav.drillIn to the backend exactly once",
    ).toEqual(["nav.drillIn"]);
    expect(
      (dispatchPayloads("nav.drillIn")[0].scopeChain as string[])[0],
      "the dispatched chain's head must be the focused column's moniker",
    ).toBe("column:col-todo");
    expect(
      spatialDrillInCalls(),
      "no legacy client-side spatial_drill_in IPC may leave the webview",
    ).toHaveLength(0);
    expect(
      spatialFocusCalls(),
      "no webview-side spatial_focus fan-out — the kernel commits focus host-side",
    ).toHaveLength(0);

    // The kernel commits focus to the remembered card and emits
    // `focus-changed`; mimic that emission and confirm the remembered
    // card flips its data-focused on the DOM side.
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

  // -------------------------------------------------------------------------
  // #6: the drill-in wire carries NO webview-built snapshot and no
  //     pre-resolved fq — the kernel pulls live geometry on demand,
  //     host-side (pinned by `builtin_nav_commands_e2e.rs`). This test
  //     replaces the legacy "drill_in IPC carries a snapshot" pin, which
  //     described the retired client-side drill wire.
  // -------------------------------------------------------------------------

  it("enter_on_focused_card_sends_no_snapshot_or_fq_on_drill_wire", async () => {
    mockKeymapMode = "vim";
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

    // Exactly one backend dispatch of the `nav.drillIn` id…
    expect(
      navDispatchCmds(mockInvoke),
      "vim Enter on a focused card must dispatch nav.drillIn to the backend exactly once",
    ).toEqual(["nav.drillIn"]);

    // …whose payload pre-resolves NOTHING: no geometry snapshot, no fq.
    // The host plugin resolves the focused scope from the kernel's
    // window slot and pulls live geometry itself.
    const payload = dispatchPayloads("nav.drillIn")[0];
    expect(
      payload.snapshot,
      "the drill wire must not carry a webview-built geometry snapshot",
    ).toBeUndefined();
    expect(
      payload.fq,
      "the drill wire must not carry a pre-resolved fq",
    ).toBeUndefined();
    expect(
      payload.focused_fq,
      "the drill wire must not carry a pre-resolved focused_fq",
    ).toBeUndefined();

    // And no legacy client-side drill IPC at all.
    expect(
      spatialDrillInCalls(),
      "no legacy client-side spatial_drill_in IPC may leave the webview",
    ).toHaveLength(0);

    unmount();
  });

  // -------------------------------------------------------------------------
  // #7a: vim Enter on a focused card drills toward the card's first
  //      field. The card → field resolution itself executes host-side
  //      in the `nav-commands` builtin plugin (pinned by the kernel-side
  //      e2e, `builtin_nav_commands_e2e.rs`); the webview's contract is
  //      routing `nav.drillIn` to the backend with the card at the head
  //      of the scope chain — and mirroring the kernel's field-focus
  //      commit when the resulting `focus-changed` arrives.
  // -------------------------------------------------------------------------

  it("enter_on_focused_card_drills_into_first_field", async () => {
    mockKeymapMode = "vim";
    const { unmount } = renderBoardWithShell();
    await flushSetup();

    const cardKey = keyForMoniker("task:t1");
    expect(
      cardKey,
      "the first card must register a spatial scope",
    ).toBeTruthy();

    await fireFocusChanged({
      next_fq: cardKey!,
      next_segment: asSegment("task:t1"),
    });
    await flushSetup();

    // The card is now the direct focus — its data-focused is set.
    await waitFor(() => {
      const t1Node = document.querySelector(
        "[data-segment='task:t1']",
      ) as HTMLElement | null;
      expect(t1Node).not.toBeNull();
      expect(t1Node!.getAttribute("data-focused")).not.toBeNull();
    });

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });
    await flushSetup();

    // Enter routes `nav.drillIn` to the backend exactly once, with the
    // focused card at the head of the dispatched scope chain — that is
    // all the context the host plugin needs to resolve the field hop.
    expect(
      navDispatchCmds(mockInvoke),
      "vim Enter on a focused card must dispatch nav.drillIn to the backend exactly once",
    ).toEqual(["nav.drillIn"]);
    expect(
      (dispatchPayloads("nav.drillIn")[0].scopeChain as string[])[0],
      "the dispatched chain's head must be the focused card's moniker",
    ).toBe("task:t1");
    expect(
      spatialDrillInCalls(),
      "no legacy client-side spatial_drill_in IPC may leave the webview",
    ).toHaveLength(0);
    expect(
      spatialFocusCalls(),
      "no webview-side spatial_focus fan-out — the kernel commits focus host-side",
    ).toHaveLength(0);

    // The kernel resolves drill_in(card) to the card's first field FQM
    // (a descendant the test schema never registers as a DOM scope) and
    // emits `focus-changed`; mimic that emission and confirm the webview
    // mirrors the hop — the card itself is no longer the direct focus.
    const fieldKey = `${cardKey}/field:title` as FullyQualifiedMoniker;
    await fireFocusChanged({
      prev_fq: cardKey!,
      next_fq: fieldKey,
      next_segment: asSegment("field:title"),
    });
    await waitFor(() => {
      const t1Node = document.querySelector(
        "[data-segment='task:t1']",
      ) as HTMLElement | null;
      expect(t1Node).not.toBeNull();
      expect(
        t1Node!.getAttribute("data-focused"),
        "focus must move INTO the field — the card must not stay the direct focus (the 'Enter does nothing' symptom)",
      ).toBeNull();
    });

    unmount();
  });

  // -------------------------------------------------------------------------
  // #7b: vim Escape on a focused field drills back to the parent card.
  //      Symmetric to #7a, mirrors the drill_out user-symptom contract.
  // -------------------------------------------------------------------------

  it("escape_on_focused_field_drills_out_to_parent_card", async () => {
    mockKeymapMode = "vim";
    const { unmount } = renderBoardWithShell();
    await flushSetup();

    const cardKey = keyForMoniker("task:t1");
    expect(cardKey).toBeTruthy();

    // Synthesize a field FQM nested under the card. The card already
    // registered a real scope on mount; the field FQM is a fabricated
    // descendant because the test schema declares no fields. The
    // field → card parent resolution executes host-side in the
    // `nav-commands` builtin plugin (`builtin_nav_commands_e2e.rs`).
    const fieldKey = `${cardKey}/field:title` as FullyQualifiedMoniker;

    // Seed focus to the field. We do NOT need to actually register the
    // field as a scope — the entity-focus bridge takes the segment from
    // the focus-changed event payload, and `extractChainBindings` walks
    // the scope chain that `<EntityFocusProvider>` produces.
    await fireFocusChanged({
      next_fq: fieldKey,
      next_segment: asSegment("field:title"),
    });
    await flushSetup();

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await act(async () => {
      fireEvent.keyDown(document, { key: "Escape", code: "Escape" });
      await Promise.resolve();
    });
    await flushSetup();

    // Escape routes the `nav.drillOut` plugin command id to the backend
    // exactly once — drill-out executes host-side; the webview sends no
    // fq, no snapshot, no client-side drill IPC, and no focus fan-out.
    expect(
      navDispatchCmds(mockInvoke),
      "vim Escape on a focused field must dispatch nav.drillOut to the backend exactly once",
    ).toEqual(["nav.drillOut"]);
    expect(
      spatialDrillOutCalls(),
      "no legacy client-side spatial_drill_out IPC may leave the webview",
    ).toHaveLength(0);
    expect(
      spatialFocusCalls(),
      "no webview-side spatial_focus fan-out — the kernel commits focus host-side",
    ).toHaveLength(0);

    // The kernel resolves the field's parent card, commits focus to it,
    // and emits `focus-changed`; mimic that emission and confirm the
    // parent card flips its data-focused back on.
    await fireFocusChanged({
      prev_fq: fieldKey,
      next_fq: cardKey!,
      next_segment: asSegment("task:t1"),
    });
    await waitFor(() => {
      const t1Node = document.querySelector(
        "[data-segment='task:t1']",
      ) as HTMLElement | null;
      expect(t1Node).not.toBeNull();
      expect(
        t1Node!.getAttribute("data-focused"),
        "drill-out must land focus back on the parent card",
      ).not.toBeNull();
    });

    unmount();
  });

  // -------------------------------------------------------------------------
  // #8: the drill-out wire carries NO webview-built snapshot and no
  //     pre-resolved fq — symmetric to #6; the kernel pulls live
  //     geometry on demand, host-side. Replaces the legacy "drill_out
  //     IPC carries a snapshot" pin from the retired client-side wire.
  // -------------------------------------------------------------------------

  it("escape_on_focused_card_sends_no_snapshot_or_fq_on_drill_wire", async () => {
    mockKeymapMode = "vim";
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
      fireEvent.keyDown(document, { key: "Escape", code: "Escape" });
      await Promise.resolve();
    });
    await flushSetup();

    // Exactly one backend dispatch of the `nav.drillOut` id…
    expect(
      navDispatchCmds(mockInvoke),
      "vim Escape on a focused card must dispatch nav.drillOut to the backend exactly once",
    ).toEqual(["nav.drillOut"]);

    // …whose payload pre-resolves NOTHING: no geometry snapshot, no fq.
    const payload = dispatchPayloads("nav.drillOut")[0];
    expect(
      payload.snapshot,
      "the drill wire must not carry a webview-built geometry snapshot",
    ).toBeUndefined();
    expect(
      payload.fq,
      "the drill wire must not carry a pre-resolved fq",
    ).toBeUndefined();
    expect(
      payload.focused_fq,
      "the drill wire must not carry a pre-resolved focused_fq",
    ).toBeUndefined();

    // And no legacy client-side drill IPC at all.
    expect(
      spatialDrillOutCalls(),
      "no legacy client-side spatial_drill_out IPC may leave the webview",
    ).toHaveLength(0);

    unmount();
  });
});
