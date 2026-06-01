/**
 * Browser-mode tests pinning the "Enter drills in, not inspect" contract on
 * the board surface.
 *
 * Covers:
 *   - vim Enter on a focused card does not dispatch `ui.inspect`.
 *   - cua Enter on a focused card does not dispatch `ui.inspect`.
 *   - cua Space on a focused card still dispatches `ui.inspect` against
 *     the focused entity via the per-`<Inspectable>` scope-level command.
 *   - vim Enter on a focused column drills into the column's first card
 *     via `spatial_drill_in`.
 *   - vim Enter on a focused column with a remembered `last_focused`
 *     drills back into that remembered card.
 *   - The drill-in / drill-out IPC payloads carry a snapshot built from
 *     the layer registry so the kernel can resolve descendants instead
 *     of short-circuiting on `snapshot=None`.
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
import { commandToolCall } from "@/test/mock-command-list";
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
  type NavSnapshot,
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
function spatialDrillInCalls(): Array<{
  fq: FullyQualifiedMoniker;
  focusedFq?: FullyQualifiedMoniker;
  snapshot?: NavSnapshot;
}> {
  return mockInvoke.mock.calls
    .filter(
      (c) =>
        c[0] === "command_tool_call" &&
        (c[1] as any)?.tool === "focus" &&
        (c[1] as any)?.op === "drill_in layer",
    )
    .map((c) => {
      // Unwrap the MCP envelope so callers see the legacy `(fq, focusedFq,
      // snapshot)` arg bag regardless of which wire the call came through.
      const raw = c[1] as Record<string, unknown>;
      const bag = (raw?.params ?? raw) as Record<string, unknown>;
      // The kernel wire renames `focusedFq` → `focused_fq` for the focus
      // server. Map it back so the legacy assertions find the field.
      if ("focused_fq" in bag && !("focusedFq" in bag)) {
        (bag as Record<string, unknown>).focusedFq = bag.focused_fq;
      }
      return bag as {
        fq: FullyQualifiedMoniker;
        focusedFq?: FullyQualifiedMoniker;
        snapshot?: NavSnapshot;
      };
    });
}

/** Pull every `spatial_drill_out` call's args, in order. */
function spatialDrillOutCalls(): Array<{
  fq: FullyQualifiedMoniker;
  focusedFq?: FullyQualifiedMoniker;
  snapshot?: NavSnapshot;
}> {
  return mockInvoke.mock.calls
    .filter(
      (c) =>
        c[0] === "command_tool_call" &&
        (c[1] as any)?.tool === "focus" &&
        (c[1] as any)?.op === "drill_out layer",
    )
    .map((c) => {
      const raw = c[1] as Record<string, unknown>;
      const bag = (raw?.params ?? raw) as Record<string, unknown>;
      if ("focused_fq" in bag && !("focusedFq" in bag)) {
        (bag as Record<string, unknown>).focusedFq = bag.focused_fq;
      }
      return bag as {
        fq: FullyQualifiedMoniker;
        focusedFq?: FullyQualifiedMoniker;
        snapshot?: NavSnapshot;
      };
    });
}

/** Filter `dispatch_command` calls down to those for `ui.inspect`. */
function inspectDispatches(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "dispatch_command")
    .map((c) => c[1] as Record<string, unknown>)
    .filter((p) => p.cmd === "ui.inspect");
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
  // #1: vim Enter on a focused card does NOT dispatch ui.inspect
  // -------------------------------------------------------------------------

  it("enter_on_focused_card_does_not_dispatch_inspect_in_vim", async () => {
    mockKeymapMode = "vim";
    const { unmount } = renderBoardWithShell();
    await flushSetup();

    // Cards register as zones — find the first task's zone key.
    const cardKey = keyForMoniker("task:t1");
    expect(cardKey, "the first card must register a spatial zone").toBeTruthy();

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
    mockInvoke.mockImplementation(
      wrapMcpDispatch(mockInvoke, async (cmd, args) => {
        if (cmd === "spatial_drill_in") {
          return "task:t1";
        }
        return defaultInvokeImpl(cmd, args);
      }) as (cmd: string, args?: unknown) => Promise<unknown>,
    );

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
      (c) =>
        c[0] === "command_tool_call" &&
        (c[1] as any)?.tool === "focus" &&
        (c[1] as any)?.op === "set focus",
    );
    expect(focusCall).toBeTruthy();
    const focusOuter = focusCall![1] as Record<string, unknown>;
    const focusArgs = ((focusOuter?.params ?? focusOuter) as { fq?: string });
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
    mockInvoke.mockImplementation(
      wrapMcpDispatch(mockInvoke, async (cmd, args) => {
        if (cmd === "spatial_drill_in") {
          return "task:t2";
        }
        return defaultInvokeImpl(cmd, args);
      }) as (cmd: string, args?: unknown) => Promise<unknown>,
    );

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });
    await flushSetup();

    const drillCalls = spatialDrillInCalls();
    expect(drillCalls.length).toBe(1);
    expect(drillCalls[0].fq).toBe(columnKey);

    const focusCall = mockInvoke.mock.calls.find(
      (c) =>
        c[0] === "command_tool_call" &&
        (c[1] as any)?.tool === "focus" &&
        (c[1] as any)?.op === "set focus",
    );
    expect(focusCall).toBeTruthy();
    const focusOuter = focusCall![1] as Record<string, unknown>;
    const focusArgs = ((focusOuter?.params ?? focusOuter) as { fq?: string });
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

  // -------------------------------------------------------------------------
  // #6: drill_in IPC carries a snapshot from the layer registry
  // -------------------------------------------------------------------------

  it("enter_on_focused_card_passes_snapshot_to_drill_in", async () => {
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

    const drillCalls = spatialDrillInCalls();
    expect(
      drillCalls.length,
      "vim Enter on a focused card must dispatch spatial_drill_in exactly once",
    ).toBe(1);
    expect(
      drillCalls[0].snapshot,
      "spatial_drill_in must carry a snapshot built from the layer registry",
    ).toBeDefined();
    const snap = drillCalls[0].snapshot!;
    expect(snap.layer_fq, "snapshot.layer_fq must be set").toBeTruthy();
    expect(
      snap.scopes.some((s) => s.fq === cardKey),
      "snapshot must include the focused card's scope entry",
    ).toBe(true);

    unmount();
  });

  // -------------------------------------------------------------------------
  // #7a: vim Enter on a focused card drills into the kernel-resolved
  //      child field FQM (the user-symptom contract).
  //
  //      The earlier #4 test pinned column → card. This pins the
  //      missing card → field hop: the React side trusts whatever FQM
  //      `spatial_drill_in` returns and forwards it to setFocus. With
  //      the kernel simulator stubbed to return a synthetic field FQM,
  //      the assertion proves the resulting `spatial_focus` IPC lands
  //      on the field — NOT back on the focused card itself (which is
  //      the visible "Enter does nothing" symptom this task fixes).
  // -------------------------------------------------------------------------

  it("enter_on_focused_card_drills_into_first_field", async () => {
    mockKeymapMode = "vim";
    const { unmount } = renderBoardWithShell();
    await flushSetup();

    const cardKey = keyForMoniker("task:t1");
    expect(cardKey, "the first card must register a spatial scope").toBeTruthy();

    await fireFocusChanged({
      next_fq: cardKey!,
      next_segment: asSegment("task:t1"),
    });
    await flushSetup();

    mockInvoke.mockClear();

    // Pretend the real kernel resolved drill_in(cardKey) to a synthetic
    // field FQM under the card. This mimics what step 12's
    // snapshot-driven `drill_in` returns for a card whose snapshot
    // contains field children (the kernel unit tests in
    // `swissarmyhammer-focus/src/navigate.rs::tests::drill_in_*` pin
    // that contract); the stub bypasses the (separate) topology
    // question of whether EntityCard registers its fields as children
    // in the snapshot.
    const fieldKey = `${cardKey}/field:title` as FullyQualifiedMoniker;
    mockInvoke.mockImplementation(
      wrapMcpDispatch(mockInvoke, async (cmd, args) => {
        if (cmd === "spatial_drill_in") {
          return fieldKey;
        }
        return defaultInvokeImpl(cmd, args);
      }) as (cmd: string, args?: unknown) => Promise<unknown>,
    );

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });
    await flushSetup();

    // Enter dispatched `nav.drillIn` for the focused card key exactly once.
    const drillCalls = spatialDrillInCalls();
    expect(
      drillCalls.length,
      "vim Enter on a focused card must dispatch spatial_drill_in exactly once",
    ).toBe(1);
    expect(drillCalls[0].fq).toBe(cardKey);

    // The success branch forwards the kernel-returned FQM to setFocus,
    // which under the production SpatialFocusProvider path invokes
    // `spatial_focus` with that FQM. If the runtime fix regressed and
    // the kernel was given no snapshot (or echoed `cardKey`), this
    // would either be the card's own FQM or never fire — both visible
    // as the user-reported "Enter does nothing" symptom.
    const focusCall = mockInvoke.mock.calls.find(
      (c) =>
        c[0] === "command_tool_call" &&
        (c[1] as any)?.tool === "focus" &&
        (c[1] as any)?.op === "set focus",
    );
    expect(
      focusCall,
      "drill-in must invoke spatial_focus on the kernel-resolved target",
    ).toBeTruthy();
    const focusOuter = focusCall![1] as Record<string, unknown>;
    const focusArgs = ((focusOuter?.params ?? focusOuter) as { fq?: string });
    expect(
      focusArgs.fq,
      "drill-in's setFocus must invoke spatial_focus with the resolved field FQM, not the card's own FQM",
    ).toBe(fieldKey);
    expect(
      focusArgs.fq,
      "regression guard: focus must NOT echo back to the focused card",
    ).not.toBe(cardKey);

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
    // kernel-simulator stub below resolves `spatial_drill_out(fieldKey)`
    // to `cardKey` exactly as the real `drill_out` would for a snapshot
    // where `parent_zone(field) == card`.
    const fieldKey = `${cardKey}/field:title` as FullyQualifiedMoniker;

    // Seed focus to the field. We do NOT need to actually register the
    // field as a scope — the entity-focus bridge takes the segment from
    // the focus-changed event payload, and `extractScopeBindings` walks
    // the scope chain that `<EntityFocusProvider>` produces.
    await fireFocusChanged({
      next_fq: fieldKey,
      next_segment: asSegment("field:title"),
    });
    await flushSetup();

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(
      wrapMcpDispatch(mockInvoke, async (cmd, args) => {
        if (cmd === "spatial_drill_out") return cardKey;
        return defaultInvokeImpl(cmd, args);
      }) as (cmd: string, args?: unknown) => Promise<unknown>,
    );

    await act(async () => {
      fireEvent.keyDown(document, { key: "Escape", code: "Escape" });
      await Promise.resolve();
    });
    await flushSetup();

    const drillCalls = spatialDrillOutCalls();
    expect(
      drillCalls.length,
      "vim Escape on a focused field must dispatch spatial_drill_out exactly once",
    ).toBe(1);

    const focusCall = mockInvoke.mock.calls.find(
      (c) =>
        c[0] === "command_tool_call" &&
        (c[1] as any)?.tool === "focus" &&
        (c[1] as any)?.op === "set focus",
    );
    expect(
      focusCall,
      "drill-out must invoke spatial_focus on the kernel-resolved parent",
    ).toBeTruthy();
    const focusOuter = focusCall![1] as Record<string, unknown>;
    const focusArgs = ((focusOuter?.params ?? focusOuter) as { fq?: string });
    expect(
      focusArgs.fq,
      "drill-out's setFocus must invoke spatial_focus with the parent card's FQM",
    ).toBe(cardKey);
    expect(
      focusArgs.fq,
      "regression guard: focus must NOT echo back to the focused field",
    ).not.toBe(fieldKey);

    unmount();
  });

  // -------------------------------------------------------------------------
  // #8: drill_out IPC carries a snapshot from the layer registry
  // -------------------------------------------------------------------------

  it("escape_on_focused_card_passes_snapshot_to_drill_out", async () => {
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
    mockInvoke.mockImplementation(
      wrapMcpDispatch(mockInvoke, async (cmd, args) => {
        if (cmd === "spatial_drill_out") return cardKey;
        return defaultInvokeImpl(cmd, args);
      }) as (cmd: string, args?: unknown) => Promise<unknown>,
    );

    await act(async () => {
      fireEvent.keyDown(document, { key: "Escape", code: "Escape" });
      await Promise.resolve();
    });
    await flushSetup();

    const drillCalls = spatialDrillOutCalls();
    expect(
      drillCalls.length,
      "vim Escape on a focused card must dispatch spatial_drill_out exactly once",
    ).toBe(1);
    expect(
      drillCalls[0].snapshot,
      "spatial_drill_out must carry a snapshot built from the layer registry",
    ).toBeDefined();

    unmount();
  });
});
