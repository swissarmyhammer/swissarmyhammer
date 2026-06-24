/**
 * Keyboard-navigation contract for `<GridView>` (browser-mode).
 *
 * Pins the fix from kanban task `01KQJDDPHB55Z4MF77YTYSAP0C`: the grid's
 * local `CommandScopeProvider` must NOT shadow the global `nav.up` /
 * `nav.down` / `nav.left` / `nav.right` commands with broadcast-no-op
 * `grid.move{Up,Down,Left,Right}` aliases. Arrow keys (and vim hjkl) inside
 * the grid must reach the global nav commands, which route the `nav.*`
 * command id to the backend — the kernel navigate executes host-side in the
 * `nav-commands` builtin plugin.
 *
 * Row-extreme keys (`Home`, `End`, `0`, `$`) and grid-extreme keys
 * (`Mod+Home`, `Mod+End`, `Shift+G`, `gg`) are tested too. The grid scope
 * keeps a small set of commands for those that route through `spatial_focus`
 * (for row-extreme — the destination cell is computed locally) or through
 * the global `nav.first`/`nav.last` commands (for `Shift+G`/`gg` which the
 * global already binds to the kernel's `first`/`last` directions).
 *
 * Mounts the grid inside `<AppShell>` so the global keybinding pipeline is
 * live (`<KeybindingHandler>` attaches a document `keydown` listener and
 * resolves bindings through the focused scope chain). Without that, the
 * `keydown` events fired in tests would land in the void.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri API mocks -- must come before component imports.
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

// Stub the perspective container so the grid gets a stable activePerspective
// without dragging in heavier providers.
vi.mock("@/components/perspective-container", () => ({
  useActivePerspective: () => ({
    activePerspective: null,
    applySort: (entities: unknown[]) => entities,
    groupField: undefined,
  }),
}));

// ---------------------------------------------------------------------------
// Imports after mocks.
// ---------------------------------------------------------------------------

import { GridView } from "./grid-view";
import { AppShell } from "./app-shell";
import { commandToolCall, navDispatchCmds } from "@/test/mock-command-list";
import {
  getWebviewCommandHandler,
  hasWebviewCommandHandler,
  resetWebviewCommandBusForTest,
} from "@/lib/webview-command-bus";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { AppModeProvider } from "@/lib/app-mode-context";
import { UndoProvider } from "@/lib/undo-context";
import { ActiveBoardPathProvider } from "@/lib/command-scope";
import { DragSessionProvider } from "@/lib/drag-session-context";
import {
  asSegment,
  type FocusChangedPayload,
  type FullyQualifiedMoniker,
  type WindowLabel,
} from "@/types/spatial";
import type { Entity, EntitySchema } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Task schema -- two columns so row-start vs row-end and left vs right are
// distinguishable (single-column grids would make those equivalent).
// ---------------------------------------------------------------------------

const TASK_SCHEMA = {
  entity: {
    name: "task",
    entity_type: "task",
    search_display_field: "title",
  },
  fields: [
    { name: "title", type: "string", section: "header", display: "text" },
    { name: "status", type: "string", section: "header", display: "text" },
  ],
} as unknown as EntitySchema;

function seedTask(id: string, title: string, status: string): Entity {
  return {
    entity_type: "task",
    id,
    moniker: `task:${id}`,
    fields: { title, status },
  };
}

function threeTasks(): Entity[] {
  return [
    seedTask("t1", "Alpha", "todo"),
    seedTask("t2", "Beta", "doing"),
    seedTask("t3", "Gamma", "done"),
  ];
}

/**
 * Mount `<GridView>` inside the production-shaped provider stack with the
 * spatial-nav layer present and `<AppShell>` wrapping so the global keymap
 * pipeline is live. The shell registers the document `keydown` handler that
 * routes presses through the focused scope chain to the global nav commands.
 */
function GridHarness({ entities }: { entities: Record<string, Entity[]> }) {
  return (
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <UIStateProvider>
            <AppModeProvider>
              <UndoProvider>
                <SchemaProvider>
                  <EntityStoreProvider entities={entities}>
                    <TooltipProvider>
                      <ActiveBoardPathProvider value="/test/board">
                        <DragSessionProvider>
                          <FieldUpdateProvider>
                            <AppShell>
                              <GridView
                                view={{
                                  id: "v-keynav",
                                  name: "Tasks",
                                  kind: "grid",
                                  entity_type: "task",
                                }}
                              />
                            </AppShell>
                          </FieldUpdateProvider>
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
    </SpatialFocusProvider>
  );
}

/** Default invoke responses for the mount-time IPCs the providers fire. */
async function defaultInvokeImpl(
  cmd: string,
  _args?: unknown,
): Promise<unknown> {
  // The global keybinding layer (arrow keys → `nav.up`/`nav.down`/…) is
  // sourced from the metadata-driven Command registry via `useCommandList`,
  // which fetches through the `command_tool_call` bridge's `list command`
  // op. Synthesize that registry from `BINDING_TABLES` so the global nav
  // bindings resolve.
  if (cmd === "command_tool_call") return commandToolCall(_args);
  if (cmd === "list_entity_types") return ["task"];
  if (cmd === "get_entity_schema") return TASK_SCHEMA;
  if (cmd === "get_ui_state")
    return {
      palette_open: false,
      palette_mode: "command",
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      windows: {},
      recent_boards: [],
    };
  if (cmd === "get_undo_state") return { can_undo: false, can_redo: false };
  if (cmd === "dispatch_command") return undefined;
  if (cmd === "list_commands_for_scope") return [];
  return undefined;
}

/** Collect every `spatial_register_scope` call payload. */
function registerScopeCalls(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_scope")
    .map((c) => c[1] as Record<string, unknown>);
}

/** Collect every `spatial_navigate` call payload, in order. */
function spatialNavigateCalls(): Array<{
  focusedFq: FullyQualifiedMoniker;
  direction: string;
}> {
  return mockInvoke.mock.calls
    .filter(
      (c) =>
        c[0] === "spatial_navigate" ||
        (c[0] === "command_tool_call" &&
          (c[1] as any)?.tool === "focus" &&
          (c[1] as any)?.op === "navigate focus"),
    )
    .map((c) => {
      const outer = c[1] as Record<string, unknown>;
      const args = (outer?.params ?? outer) as {
        focusedFq: FullyQualifiedMoniker;
        direction: string;
      };
      return args;
    });
}

/** Collect every `spatial_focus` call payload, in order. */
function spatialFocusCalls(): Array<{ fq: FullyQualifiedMoniker }> {
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
      const args = (outer?.params ?? outer) as { fq: FullyQualifiedMoniker };
      return args;
    });
}

/**
 * Wait for register effects scheduled inside `useEffect` to flush.
 *
 * Several settle steps run after mount: `<UIStateProvider>` resolves
 * `get_ui_state`, the spatial primitives' register-zone/scope effects fire,
 * and `<KeybindingHandler>` attaches its `listen("menu-command", …)`
 * subscription. A 50ms `setTimeout` is enough for all of them.
 */
async function flushSetup() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 50));
  });
}

/**
 * Drive a `focus-changed` event into the React tree as if the Rust kernel
 * had emitted one for the active window.
 *
 * `next_segment` is required: the entity-focus bridge mirrors it into the
 * legacy entity-focus store so the focused scope chain (used by
 * `<KeybindingHandler>` to resolve scope-level bindings) is populated.
 * Without that the global nav commands would be visible to the keymap but
 * `actions.focusedFq()` would still work — what matters here is that the
 * spatial provider's internal `focusedFqRef` is updated, which IS what
 * `next_fq` triggers regardless of `next_segment`.
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
    next_segment: next_segment === null ? null : asSegment(next_segment),
  };
  const handlers = listeners.get("notifications/focus/changed") ?? [];
  await act(async () => {
    for (const handler of handlers) handler({ payload });
    await Promise.resolve();
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("GridView keyboard navigation (spatial)", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    mockInvoke.mockImplementation(defaultInvokeImpl);
    resetWebviewCommandBusForTest();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Helper: mount the harness, wait for setup, seed focus on the target
   * cell, and return the cell's FQM. Tests use this to centralise the
   * arrange step before driving keystrokes.
   */
  async function mountAndSeedFocus(targetMoniker: string) {
    const entities = { task: threeTasks() };
    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(<GridHarness entities={entities} />);
    });
    await flushSetup();

    const cellRegistration = registerScopeCalls().find(
      (c) => c.segment === targetMoniker,
    );
    expect(
      cellRegistration,
      `cell ${targetMoniker} must register before seeding focus`,
    ).toBeTruthy();
    const cellKey = cellRegistration!.fq as FullyQualifiedMoniker;

    await fireFocusChanged({
      next_fq: cellKey,
      next_segment: targetMoniker,
    });

    return { result, cellKey };
  }

  // -------------------------------------------------------------------------
  // Cardinal arrow keys — must reach the global nav.up/down/left/right
  // commands, which route the command id to the backend. The kernel
  // navigate executes host-side in the `nav-commands` builtin plugin (it
  // resolves the focused scope and pulls the live geometry from the
  // webview itself), so the webview sends NO client-side `navigate focus`
  // IPC and no focused fq.
  // -------------------------------------------------------------------------

  it("ArrowDown dispatches nav.down to the backend for the focused cell", async () => {
    await mountAndSeedFocus("grid_cell:0:title");

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await act(async () => {
      fireEvent.keyDown(document, { key: "ArrowDown" });
      await Promise.resolve();
    });

    expect(navDispatchCmds(mockInvoke)).toEqual(["nav.down"]);
    // No legacy client-side navigate IPC — the kernel move is host-driven.
    expect(spatialNavigateCalls()).toHaveLength(0);
  });

  it("ArrowUp dispatches nav.up to the backend for the focused cell", async () => {
    await mountAndSeedFocus("grid_cell:1:title");

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await act(async () => {
      fireEvent.keyDown(document, { key: "ArrowUp" });
      await Promise.resolve();
    });

    expect(navDispatchCmds(mockInvoke)).toEqual(["nav.up"]);
    expect(spatialNavigateCalls()).toHaveLength(0);
  });

  it("ArrowLeft dispatches nav.left to the backend for the focused cell", async () => {
    await mountAndSeedFocus("grid_cell:0:status");

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await act(async () => {
      fireEvent.keyDown(document, { key: "ArrowLeft" });
      await Promise.resolve();
    });

    expect(navDispatchCmds(mockInvoke)).toEqual(["nav.left"]);
    expect(spatialNavigateCalls()).toHaveLength(0);
  });

  it("ArrowRight dispatches nav.right to the backend for the focused cell", async () => {
    await mountAndSeedFocus("grid_cell:0:title");

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await act(async () => {
      fireEvent.keyDown(document, { key: "ArrowRight" });
      await Promise.resolve();
    });

    expect(navDispatchCmds(mockInvoke)).toEqual(["nav.right"]);
    expect(spatialNavigateCalls()).toHaveLength(0);
  });

  // -------------------------------------------------------------------------
  // Row-extreme bindings — Home/End in cua mode should jump to the first or
  // last cell of the current row. The grid scope owns these commands and
  // routes them through the spatial-nav kernel via setFocus (not the
  // broadcast-no-op).
  // -------------------------------------------------------------------------

  it("Home dispatches spatial_focus to the first cell of the current row", async () => {
    // Seed focus on (row=1, col=status) so Home should move to (row=1, col=title).
    const { result } = await mountAndSeedFocus("grid_cell:1:status");

    // Capture the destination cell's key BEFORE clearing the mock.
    const targetCell = registerScopeCalls().find(
      (c) => c.segment === "grid_cell:1:title",
    );
    expect(targetCell).toBeTruthy();
    const targetKey = targetCell!.fq as FullyQualifiedMoniker;

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await act(async () => {
      fireEvent.keyDown(document, { key: "Home" });
      await Promise.resolve();
    });

    // Home in the grid is a row-extreme move — it routes through
    // `setFocus(composeFq(gridZoneFq, asSegment("grid_cell:1:title")))`,
    // which the entity-focus bridge dispatches to the kernel as a single
    // `spatial_focus` IPC. There must be no `spatial_navigate` call.
    const focusCalls = spatialFocusCalls();
    expect(focusCalls).toHaveLength(1);
    expect(focusCalls[0].fq).toBe(targetKey);

    expect(spatialNavigateCalls()).toHaveLength(0);

    result.unmount();
  });

  it("End dispatches spatial_focus to the last cell of the current row", async () => {
    // Seed focus on (row=2, col=title) so End should move to (row=2, col=status).
    const { result } = await mountAndSeedFocus("grid_cell:2:title");

    const targetCell = registerScopeCalls().find(
      (c) => c.segment === "grid_cell:2:status",
    );
    expect(targetCell).toBeTruthy();
    const targetKey = targetCell!.fq as FullyQualifiedMoniker;

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await act(async () => {
      fireEvent.keyDown(document, { key: "End" });
      await Promise.resolve();
    });

    const focusCalls = spatialFocusCalls();
    expect(focusCalls).toHaveLength(1);
    expect(focusCalls[0].fq).toBe(targetKey);

    expect(spatialNavigateCalls()).toHaveLength(0);

    result.unmount();
  });

  // -------------------------------------------------------------------------
  // Grid-extreme bindings — Mod+Home/Mod+End should jump to the absolute
  // first/last cell of the grid.
  // -------------------------------------------------------------------------

  it("Mod+Home dispatches spatial_focus to the absolute first cell of the grid", async () => {
    // Seed focus on (row=2, col=status) so Mod+Home should move to (0, title).
    const { result } = await mountAndSeedFocus("grid_cell:2:status");

    const targetCell = registerScopeCalls().find(
      (c) => c.segment === "grid_cell:0:title",
    );
    expect(targetCell).toBeTruthy();
    const targetKey = targetCell!.fq as FullyQualifiedMoniker;

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await act(async () => {
      // `metaKey` is the Cmd modifier on macOS. In the keybinding
      // normalizer Mod+Home matches either Cmd+Home (macOS) or Ctrl+Home
      // (other OSes). The browser test harness reports macOS, so metaKey
      // is the right modifier for Mod.
      fireEvent.keyDown(document, { key: "Home", metaKey: true });
      await Promise.resolve();
    });

    const focusCalls = spatialFocusCalls();
    expect(focusCalls).toHaveLength(1);
    expect(focusCalls[0].fq).toBe(targetKey);

    expect(spatialNavigateCalls()).toHaveLength(0);

    result.unmount();
  });

  it("Mod+End dispatches spatial_focus to the absolute last cell of the grid", async () => {
    // Seed focus on (row=0, col=title) so Mod+End should move to (last, last).
    const { result } = await mountAndSeedFocus("grid_cell:0:title");

    const targetCell = registerScopeCalls().find(
      (c) => c.segment === "grid_cell:2:status",
    );
    expect(targetCell).toBeTruthy();
    const targetKey = targetCell!.fq as FullyQualifiedMoniker;

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await act(async () => {
      fireEvent.keyDown(document, { key: "End", metaKey: true });
      await Promise.resolve();
    });

    const focusCalls = spatialFocusCalls();
    expect(focusCalls).toHaveLength(1);
    expect(focusCalls[0].fq).toBe(targetKey);

    expect(spatialNavigateCalls()).toHaveLength(0);

    result.unmount();
  });

  // -------------------------------------------------------------------------
  // Negative invariant — no broadcast-style code path
  //
  // The bug under fix: `buildGridNavCommands` registered shadow `grid.move*`
  // commands whose `execute` called `broadcastRef.current(navEvent)`, which
  // resolves to the no-op `broadcastNavCommand` in `entity-focus-context`.
  // After the fix, no arrow keystroke inside the grid should route through
  // any local grid command for cardinal directions — the global `nav.up` /
  // `nav.down` / `nav.left` / `nav.right` should win.
  //
  // The behavioural fingerprint of the broken path is: arrow key fires, no
  // `nav.*` dispatch lands, no `spatial_focus` IPC lands, the cell cursor
  // doesn't move. The cardinal-direction tests above already assert the
  // positive (the global `nav.*` command id reaches the backend); this test
  // additionally pins that the same key produces zero broadcast-call side
  // effects in the IPC log (no `dispatch_command` with a `grid.move*` cmd
  // shape).
  // -------------------------------------------------------------------------

  it("arrow keys do not dispatch any grid.move* command (no shadow registration)", async () => {
    await mountAndSeedFocus("grid_cell:0:title");

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await act(async () => {
      fireEvent.keyDown(document, { key: "ArrowDown" });
      fireEvent.keyDown(document, { key: "ArrowRight" });
      fireEvent.keyDown(document, { key: "ArrowUp" });
      fireEvent.keyDown(document, { key: "ArrowLeft" });
      await Promise.resolve();
    });

    // Every dispatch_command with a grid.move{Up,Down,Left,Right} cmd is a
    // regression — the shadow command path is back.
    const gridMoveDispatches = mockInvoke.mock.calls.filter((c) => {
      if (c[0] !== "dispatch_command") return false;
      const cmd = (c[1] as { cmd?: string } | undefined)?.cmd;
      return (
        cmd === "grid.moveUp" ||
        cmd === "grid.moveDown" ||
        cmd === "grid.moveLeft" ||
        cmd === "grid.moveRight"
      );
    });
    expect(
      gridMoveDispatches.map((c) => (c[1] as { cmd?: string }).cmd),
      "grid.move{Up,Down,Left,Right} must not be dispatched on arrow keys",
    ).toEqual([]);

    // And the global nav commands must have fired four times (one per
    // arrow), each routed to the backend as its `nav.*` command id.
    expect(navDispatchCmds(mockInvoke)).toEqual([
      "nav.down",
      "nav.right",
      "nav.up",
      "nav.left",
    ]);
    // Host-driven nav: the webview sends no client-side navigate IPC.
    expect(spatialNavigateCalls()).toHaveLength(0);
  });

  // -------------------------------------------------------------------------
  // Webview command bus — Card C. The eleven `grid.*` commands are DEFINED
  // by the `grid-commands` builtin plugin (id / name / keys / scope:
  // ["ui:grid"]); grid-view.tsx registers a live webview-bus handler per id
  // on mount (`registerWebviewCommandHandler`). Dispatching an id —
  // keybinding, palette, or programmatic — runs the bus handler, never a
  // client-side `CommandDef` execute (none exists anymore) and never a
  // backend `grid.*` dispatch.
  // -------------------------------------------------------------------------

  /** The eleven plugin-owned grid command ids (Card C). */
  const GRID_COMMAND_IDS = [
    "grid.moveToRowStart",
    "grid.moveToRowEnd",
    "grid.firstCell",
    "grid.lastCell",
    "grid.edit",
    "grid.editEnter",
    "grid.exitEdit",
    "grid.toggleVisual",
    "grid.deleteRow",
    "grid.newBelow",
    "grid.newAbove",
  ];

  /** Run a grid id's registered webview-bus handler inside act(). */
  async function runBusHandler(id: string) {
    const handler = getWebviewCommandHandler(id);
    expect(
      handler,
      `webview-bus handler for ${id} must be registered`,
    ).toBeTruthy();
    await act(async () => {
      await handler!({});
      await Promise.resolve();
    });
  }

  /** Collect every backend dispatch_command cmd, in call order. */
  function dispatchedCmds(): string[] {
    return mockInvoke.mock.calls
      .filter((c) => c[0] === "dispatch_command")
      .map((c) => (c[1] as { cmd?: string } | undefined)?.cmd ?? "");
  }

  describe("grid.* commands route through the webview command bus", () => {
    it("registers a webview-bus handler for all 11 grid.* ids on mount", async () => {
      await mountAndSeedFocus("grid_cell:0:title");
      for (const id of GRID_COMMAND_IDS) {
        expect(
          hasWebviewCommandHandler(id),
          `${id} must have a registered webview-bus handler`,
        ).toBe(true);
      }
    });

    it("Enter enters edit mode via the bus-handled grid.edit (no backend grid dispatch)", async () => {
      const { result } = await mountAndSeedFocus("grid_cell:0:title");

      mockInvoke.mockClear();
      mockInvoke.mockImplementation(defaultInvokeImpl);

      await act(async () => {
        fireEvent.keyDown(document, { key: "Enter" });
        await Promise.resolve();
      });

      // The live grid behavior ran: the status bar flips to EDIT.
      expect(result.container.textContent).toContain("EDIT");
      // The bus short-circuits the backend: no grid.* id reaches
      // dispatch_command (and Enter did not fall through to nav.drillIn).
      expect(dispatchedCmds()).toEqual([]);
    });

    it("grid.toggleVisual and grid.exitEdit drive the live grid mode via the bus", async () => {
      const { result } = await mountAndSeedFocus("grid_cell:0:title");

      await runBusHandler("grid.toggleVisual");
      expect(result.container.textContent).toContain("VISUAL");

      // grid.exitEdit exits visual mode too (the React def's contract).
      await runBusHandler("grid.exitEdit");
      expect(result.container.textContent).toContain("NORMAL");

      // And the edit-mode pair: editEnter enters, exitEdit returns to normal.
      await runBusHandler("grid.editEnter");
      expect(result.container.textContent).toContain("EDIT");
      await runBusHandler("grid.exitEdit");
      expect(result.container.textContent).toContain("NORMAL");
    });

    it("grid.deleteRow re-dispatches entity.archive targeting the cursor row's moniker", async () => {
      // Seed focus on row 1 — the cursor derives from the focused moniker,
      // so deleteRow must archive the SECOND task (t2). The registered
      // backend command is the cross-cutting `entity.archive` (resolving
      // `from: target`); no per-type `{type}.archive` command exists, so
      // dispatching one would silently fail in production.
      await mountAndSeedFocus("grid_cell:1:title");

      mockInvoke.mockClear();
      mockInvoke.mockImplementation(defaultInvokeImpl);

      await runBusHandler("grid.deleteRow");

      const archiveCalls = mockInvoke.mock.calls.filter(
        (c) =>
          c[0] === "dispatch_command" &&
          (c[1] as { cmd?: string } | undefined)?.cmd === "entity.archive",
      );
      expect(archiveCalls).toHaveLength(1);
      expect(
        (archiveCalls[0][1] as { target?: string }).target,
        "entity.archive resolves its entity from the target moniker",
      ).toBe("task:t2");
    });

    it("grid.newBelow and grid.newAbove re-dispatch entity.add:task through the backend", async () => {
      await mountAndSeedFocus("grid_cell:0:title");

      mockInvoke.mockClear();
      mockInvoke.mockImplementation(defaultInvokeImpl);

      await runBusHandler("grid.newBelow");
      await runBusHandler("grid.newAbove");

      expect(dispatchedCmds()).toEqual(["entity.add:task", "entity.add:task"]);
    });

    it("grid.moveToRowEnd jumps to the last cell of the cursor row via the bus", async () => {
      const { result } = await mountAndSeedFocus("grid_cell:1:title");

      const targetCell = registerScopeCalls().find(
        (c) => c.segment === "grid_cell:1:status",
      );
      expect(targetCell).toBeTruthy();
      const targetKey = targetCell!.fq as FullyQualifiedMoniker;

      mockInvoke.mockClear();
      mockInvoke.mockImplementation(defaultInvokeImpl);

      await runBusHandler("grid.moveToRowEnd");

      const focusCalls = spatialFocusCalls();
      expect(focusCalls).toHaveLength(1);
      expect(focusCalls[0].fq).toBe(targetKey);

      result.unmount();
    });
  });
});
