/**
 * Browser-mode test for `<BoardView>`'s spatial-nav behaviour.
 *
 * Source of truth for acceptance of card `01KNQXZ81Q...`. The board zone is
 * viewport-sized chrome — it registers in the spatial graph (so the
 * navigator can drill into and out of it) but intentionally does NOT
 * render a visible focus bar around the entire viewport. This file pins
 * every contract `BoardSpatialZone` carries:
 *
 *   1. Registration via `spatial_register_scope` with moniker `board:board-1`.
 *   2. Click on the board chrome → `spatial_focus(boardKey)` with
 *      stop-propagation so the click does not bubble up to a window-root
 *      ancestor and does not leak down into a column zone.
 *   3. Focus claim flips `data-focused` for e2e selectors but does NOT
 *      mount `<FocusIndicator>` (because `showFocus={false}` — see the
 *      inline comment on `BoardSpatialZone`).
 *   4. Keystrokes route through the AppShell's global nav commands and
 *      dispatch `spatial_navigate(boardKey, direction)` for arrows and
 *      hjkl.
 *   5. Enter dispatches `spatial_drill_in(boardKey)`; after the kernel
 *      resolves a child column moniker the column flips its
 *      `data-focused`.
 *   6. Unmount unregisters via `spatial_unregister_scope`.
 *   7. Legacy nav names (`entity_focus_*`, `claim_when_*`,
 *      `broadcast_nav_*`) appear nowhere in the IPC trace.
 *
 * Plus an integration test for the drill-out chain card → column →
 * board → window-root layer that the umbrella card `01KQ5PEHWT...`
 * mandates as the systemic verification step for the spatial-nav
 * project.
 *
 * Mock pattern matches `grid-view.nav-is-eventdriven.test.tsx` and the
 * sister browser-mode test `perspective-view.spatial.test.tsx`:
 * `vi.hoisted` builds an invoke / listen mock pair the test owns;
 * `mockListen` records every `listen("focus-changed", cb)` callback so
 * `fireFocusChanged(key)` can drive the React tree as if the Rust
 * kernel had emitted the event.
 *
 * Runs under the browser project (real Chromium via Playwright) — every
 * `*.test.tsx` outside `*.node.test.tsx` lands there per `vite.config.ts`.
 *
 * # Tab / Shift+Tab
 *
 * Tab cycles the focused board's columns to the right (`nav.right`)
 * and Shift+Tab cycles to the left (`nav.left`). Both bindings live in
 * `BINDING_TABLES.cua` and route through the same `nav.right`/
 * `nav.left` execute closures that arrow keys hit, so the board-view
 * contract is identical: a keystroke dispatches
 * `spatial_navigate(boardKey, direction)`.
 *
 * Followup task `01KQ7CQNFJ...` (Distinguish Shift+Tab from Tab in
 * keybinding normalizer) added the Shift+ prefix for symbolic keys to
 * `normalizeKeyEvent`; without it Shift+Tab and Tab hashed to the same
 * canonical string and the two bindings could not be registered
 * distinctly.
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
// Default invoke responses — the handful of IPCs the AppShell + BoardView
// providers hit on mount. Kept in one place so beforeEach restores them
// cleanly after each test's mockClear / mockReset.
// ---------------------------------------------------------------------------

async function defaultInvokeImpl(
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
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      windows: {},
      recent_boards: [],
    };
  if (cmd === "get_undo_state") return { can_undo: false, can_redo: false };
  if (cmd === "dispatch_command") return undefined;
  // The spatial-nav register/unregister/focus/navigate calls all return
  // void — undefined is the safe default.
  return undefined;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Wait for register effects scheduled in `useEffect` to flush.
 *
 * The provider stack mounted by `renderBoardWithShell` triggers several
 * async settle steps:
 *   1. `<UIStateProvider>` fetches `get_ui_state` and waits for the
 *      promise to resolve.
 *   2. The spatial primitives' `useEffect` register hooks fire after
 *      paint and call `spatial_register_scope` / `spatial_register_scope`.
 *   3. `<KeybindingHandler>`'s `listen("menu-command", …)` and
 *      `listen("context-menu-command", …)` resolve.
 *   4. The virtualizer in `<ColumnView>` measures its container and
 *      schedules a second pass.
 *
 * One microtask flush is not enough to settle all four. We give it a
 * short setTimeout (50ms) — the same nudge `grid-view.nav-is-eventdriven.test.tsx`
 * uses for the same reason.
 */
async function flushSetup() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 50));
  });
}

/**
 * Drive a `focus-changed` event into the React tree, mimicking the Rust
 * kernel emitting one for the active window.
 *
 * The provider's listener decides which side of the swap fires — we
 * always pass both `prev_fq` and `next_fq` to mimic the kernel's
 * payload shape. Wrapping the dispatch in `act()` flushes the React
 * state updates so the caller can assert against post-update DOM in
 * the next tick.
 *
 * The `next_segment` argument is REQUIRED for keystroke tests: the
 * spatial→entity bridge in `<EntityFocusProvider>` calls
 * `actions.setFocus(payload.next_segment)` on every focus-changed
 * event. The entity-focus store's `focusedScope` is what AppShell's
 * `<KeybindingHandler>` walks via `extractScopeBindings` to resolve
 * scope-level command keys (including `nav.up`/`nav.down`/`nav.left`/
 * `nav.right`'s `keys.cua` arrow bindings). When `next_segment` is
 * null, the entity-focus store is cleared, `focusedScope` becomes
 * null, and arrow keys never resolve to a command.
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
 * That is what wires arrow keys / hjkl to the global `nav.up`/`nav.down`/
 * `nav.left`/`nav.right` commands whose `execute` closures invoke
 * `spatial_navigate` against the currently-focused `FullyQualifiedMoniker`. Without
 * the AppShell those keystrokes would land in the void.
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

/** Pull every `spatial_focus` call's args, in order. */
function spatialFocusCalls(): Array<{ fq: FullyQualifiedMoniker }> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_focus")
    .map((c) => c[1] as { fq: FullyQualifiedMoniker });
}

/** Pull every `spatial_navigate` call's args, in order. */
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

/** Pull every `spatial_drill_in` call's args, in order. */
function spatialDrillInCalls(): Array<{ fq: FullyQualifiedMoniker }> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_drill_in")
    .map((c) => c[1] as { fq: FullyQualifiedMoniker });
}

/** Pull every `spatial_unregister_scope` call's args, in order. */
function unregisterScopeCalls(): Array<{ fq: FullyQualifiedMoniker }> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_unregister_scope")
    .map((c) => c[1] as { fq: FullyQualifiedMoniker });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("BoardView — browser spatial behaviour", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    mockInvoke.mockImplementation(defaultInvokeImpl);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("registers a board:board-1 entity zone on mount (test #1)", async () => {
    const { unmount } = renderBoardWithShell();
    await flushSetup();

    // Post-`8232b25cc`, the redundant `ui:board` chrome zone was
    // dropped — the board content mounts directly under the
    // `board:{id}` entity zone (the `<Inspectable>` + `<FocusScope>`
    // pair on `<BoardView>`). The entity zone is registered exactly
    // once at mount time.
    const boardZones = registerScopeArgs().filter(
      (a) => a.segment === "board:board-1",
    );
    expect(boardZones).toHaveLength(1);
    const boardZone = boardZones[0];
    expect(typeof boardZone.fq).toBe("string");
    expect(boardZone.layerFq).toBeTruthy();

    // The chrome `ui:board` scope must NOT register — its removal was
    // the whole point of `8232b25cc`. A regression that re-introduces
    // it would re-create the same-rect overlap warning.
    const chromeScopes = registerScopeArgs().filter(
      (a) => a.segment === "ui:board",
    );
    expect(chromeScopes).toHaveLength(0);

    unmount();
  });

  it("clicking the board chrome dispatches spatial_focus for the board key (test #2)", async () => {
    const { container, unmount } = renderBoardWithShell();
    await flushSetup();

    const boardZone = registerScopeArgs().find(
      (a) => a.segment === "board:board-1",
    )!;
    const boardKey = boardZone.fq as FullyQualifiedMoniker;
    const boardNode = container.querySelector(
      "[data-segment='board:board-1']",
    ) as HTMLElement;
    expect(boardNode).not.toBeNull();

    // Reset invoke after mount so we measure only the click's IPC.
    mockInvoke.mockClear();

    fireEvent.click(boardNode);

    const focusCalls = spatialFocusCalls();
    // Exactly one `spatial_focus` for the board key — `e.stopPropagation()`
    // inside `<FocusScope>`'s click handler keeps the event from bubbling
    // up to a window-root ancestor and ensures only this zone's key is
    // sent. The card description's regression note: clicking the board
    // chrome must not also fire `spatial_focus` for an inner column
    // unless the click was on the column.
    expect(focusCalls).toHaveLength(1);
    expect(focusCalls[0].fq).toBe(boardKey);

    unmount();
  });

  it("focus claim on the board zone flips data-focused but renders no indicator (test #3)", async () => {
    // The board fills the viewport — drawing a focus rectangle around
    // the entire board body would be visual noise, so
    // `BoardSpatialZone` passes `showFocus={false}` to the zone (see
    // the inline comment in `board-view.tsx`). The data-focused
    // attribute must still flip so e2e tooling and the umbrella card's
    // verification protocol can observe the claim.
    const { container, queryByTestId, unmount } = renderBoardWithShell();
    await flushSetup();

    const boardZone = registerScopeArgs().find(
      (a) => a.segment === "board:board-1",
    )!;
    const boardKey = boardZone.fq as FullyQualifiedMoniker;
    const boardNode = container.querySelector(
      "[data-segment='board:board-1']",
    ) as HTMLElement;
    expect(boardNode).not.toBeNull();
    expect(boardNode.getAttribute("data-focused")).toBeNull();

    await fireFocusChanged({ next_fq: boardKey });

    await waitFor(() => {
      expect(boardNode.getAttribute("data-focused")).not.toBeNull();
    });

    // The board zone uses `showFocus={false}`, so even though the
    // data-focused attribute flips, no `<FocusIndicator>` should mount
    // as a direct descendant of the board zone. (Inner sized leaves
    // and entities — columns, cards — render their own indicators
    // when focused; this assertion only verifies the board zone
    // itself doesn't.)
    const indicatorsInsideBoard = boardNode.querySelectorAll(
      "[data-testid='focus-indicator']",
    );
    // Filter out indicators that belong to inner zones (columns may
    // also be data-focused if e.g. a phantom focus event arrived
    // earlier). Indicators that are *direct* descendants of the board
    // zone — i.e. not inside another `data-moniker` element — would
    // be the violators.
    const directBoardIndicators = Array.from(indicatorsInsideBoard).filter(
      (el) => {
        const closestMoniker = el.parentElement?.closest("[data-segment]");
        return closestMoniker === boardNode;
      },
    );
    expect(directBoardIndicators).toHaveLength(0);

    // Belt-and-suspenders: when the test page contains no other
    // focus-claim sources, no indicator at all should be visible.
    expect(queryByTestId("focus-indicator")).toBeNull();

    unmount();
  });

  it("arrow keys and hjkl dispatch spatial_navigate for the focused board (test #4)", async () => {
    const { unmount } = renderBoardWithShell();
    await flushSetup();

    const boardZone = registerScopeArgs().find(
      (a) => a.segment === "board:board-1",
    )!;
    const boardKey = boardZone.fq as FullyQualifiedMoniker;

    // Seed the spatial focus so `nav.up/down/left/right`'s execute
    // closures see a non-null `focusedKey()` and dispatch
    // `spatial_navigate` with the board's key. Without this seed the
    // commands short-circuit (focused key is null on a fresh
    // SpatialFocusProvider).
    //
    // `next_segment` is also seeded so the entity-focus bridge mirrors
    // the focused moniker into the entity-focus store. The
    // `<KeybindingHandler>` resolves scope-level command bindings
    // (`nav.up.keys.cua = "ArrowUp"` etc.) by walking the focused
    // entity scope chain — when no entity is focused, no scope-level
    // bindings are visible and arrow keys never resolve.
    await fireFocusChanged({
      next_fq: boardKey,
      next_segment: asSegment("board:board-1"),
    });

    mockInvoke.mockClear();

    // Each pair of (key, direction) covers one CUA arrow key. The
    // keybinding pipeline canonicalises the event via
    // `normalizeKeyEvent` and resolves through the active keymap mode
    // (`cua` by default in `defaultInvokeImpl`). Vim hjkl letters bind
    // through `CommandDef.keys.vim` and only fire when the keymap mode
    // is "vim" — that path is exercised end-to-end by
    // `app-shell.test.tsx` and `keybindings.test.ts`. The board's own
    // contract is "when these nav commands fire, they dispatch
    // `spatial_navigate(boardKey, direction)`" — proven by the four
    // CUA expectations below.
    const arrowExpectations: Array<{ key: string; direction: string }> = [
      { key: "ArrowUp", direction: "up" },
      { key: "ArrowDown", direction: "down" },
      { key: "ArrowLeft", direction: "left" },
      { key: "ArrowRight", direction: "right" },
    ];

    for (const { key, direction } of arrowExpectations) {
      mockInvoke.mockClear();
      await act(async () => {
        fireEvent.keyDown(document, { key });
        await Promise.resolve();
      });

      const navCalls = spatialNavigateCalls().filter(
        (c) => c.focusedFq === boardKey,
      );
      expect(navCalls.length, `${key} should dispatch spatial_navigate`).toBe(
        1,
      );
      expect(navCalls[0].direction).toBe(direction);
    }

    unmount();
  });

  it("Tab and Shift+Tab dispatch spatial_navigate right/left for the focused board", async () => {
    // Mirrors the arrow-key test above: with the board zone focused,
    // Tab routes through the global `BINDING_TABLES.cua` mapping to
    // `nav.right` and dispatches `spatial_navigate(boardKey, "right")`;
    // Shift+Tab routes to `nav.left` and dispatches `"left"`. The
    // distinction is only possible because `normalizeKeyEvent` now
    // prefixes `Shift+` on symbolic keys (task `01KQ7CQNFJ...`).
    const { unmount } = renderBoardWithShell();
    await flushSetup();

    const boardZone = registerScopeArgs().find(
      (a) => a.segment === "board:board-1",
    )!;
    const boardKey = boardZone.fq as FullyQualifiedMoniker;

    // Seed focus so `nav.right`/`nav.left`'s execute closures see a
    // non-null `focusedKey()` — same setup as the arrow-key test.
    await fireFocusChanged({
      next_fq: boardKey,
      next_segment: asSegment("board:board-1"),
    });

    const tabExpectations: Array<{
      key: string;
      shiftKey: boolean;
      direction: string;
    }> = [
      { key: "Tab", shiftKey: false, direction: "right" },
      { key: "Tab", shiftKey: true, direction: "left" },
    ];

    for (const { key, shiftKey, direction } of tabExpectations) {
      mockInvoke.mockClear();
      await act(async () => {
        fireEvent.keyDown(document, { key, shiftKey });
        await Promise.resolve();
      });

      const navCalls = spatialNavigateCalls().filter(
        (c) => c.focusedFq === boardKey,
      );
      const label = shiftKey ? `Shift+${key}` : key;
      expect(navCalls.length, `${label} should dispatch spatial_navigate`).toBe(
        1,
      );
      expect(navCalls[0].direction).toBe(direction);
    }

    unmount();
  });

  it("Enter dispatches spatial_drill_in for the focused board (test #5)", async () => {
    const { unmount } = renderBoardWithShell();
    await flushSetup();

    const boardZone = registerScopeArgs().find(
      (a) => a.segment === "board:board-1",
    )!;
    const boardKey = boardZone.fq as FullyQualifiedMoniker;
    // Capture the column key BEFORE clearing the mock — the column
    // registered during mount and that record lives in
    // `mockInvoke.mock.calls` until we clear it.
    const todoColumn = registerScopeArgs().find(
      (a) => a.segment === "column:col-todo",
    );
    expect(todoColumn).toBeTruthy();
    const todoColumnKey = todoColumn!.fq as FullyQualifiedMoniker;

    // Seed the focus so `nav.drillIn`'s execute closure sees a non-null
    // focused key. The closure hands that key to `spatial_drill_in`
    // directly. `next_segment` is also seeded so the keybinding handler
    // resolves Enter to `nav.drillIn` via the focused scope's bindings
    // (Enter is bound globally too, but the contract under test is the
    // scope-level resolution).
    await fireFocusChanged({
      next_fq: boardKey,
      next_segment: asSegment("board:board-1"),
    });

    mockInvoke.mockClear();

    // Arrange: when Enter fires, `nav.drillIn` calls `spatial_drill_in`,
    // awaits its result, and (when non-null) dispatches `setFocus` for
    // the returned moniker. Hand back a column moniker so the
    // entity-focus bridge surface in `EntityFocusProvider` mirrors it
    // into the entity-focus store and the column flips its
    // `data-focused` after the next focus-changed event.
    mockInvoke.mockImplementation(async (cmd, args) => {
      if (cmd === "spatial_drill_in") {
        return "column:col-todo";
      }
      return defaultInvokeImpl(cmd, args);
    });

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter" });
      await Promise.resolve();
    });

    const drillCalls = spatialDrillInCalls();
    expect(drillCalls).toHaveLength(1);
    expect(drillCalls[0].fq).toBe(boardKey);

    await fireFocusChanged({
      prev_fq: boardKey,
      next_fq: todoColumnKey,
    });

    await waitFor(() => {
      const todoNode = document.querySelector(
        "[data-segment='column:col-todo']",
      ) as HTMLElement;
      expect(todoNode).not.toBeNull();
      expect(todoNode.getAttribute("data-focused")).not.toBeNull();
    });

    unmount();
  });

  it("unmount unregisters the board zone via spatial_unregister_scope (test #6)", async () => {
    const { unmount } = renderBoardWithShell();
    await flushSetup();

    const boardZone = registerScopeArgs().find(
      (a) => a.segment === "board:board-1",
    )!;
    const boardKey = boardZone.fq as FullyQualifiedMoniker;

    mockInvoke.mockClear();
    unmount();
    await flushSetup();

    const unregisterKeys = unregisterScopeCalls().map((c) => c.fq);
    expect(unregisterKeys).toContain(boardKey);
  });

  it("emits no legacy entity_focus_* / claim_when_* / broadcast_nav_* IPCs (test #7)", async () => {
    const { container, unmount } = renderBoardWithShell();
    await flushSetup();

    // Exercise mount + click + a focus claim — the three lifecycle
    // points where legacy code would have called the banned commands.
    const boardZone = registerScopeArgs().find(
      (a) => a.segment === "board:board-1",
    )!;
    const boardKey = boardZone.fq as FullyQualifiedMoniker;
    const boardNode = container.querySelector(
      "[data-segment='board:board-1']",
    ) as HTMLElement;
    fireEvent.click(boardNode);
    await fireFocusChanged({ next_fq: boardKey });

    const banned = /^(entity_focus_|claim_when_|broadcast_nav_)/;
    const offenders = mockInvoke.mock.calls
      .map((c) => c[0])
      .filter((cmd) => typeof cmd === "string" && banned.test(cmd as string));
    expect(offenders).toEqual([]);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Drill-out chain integration test — the umbrella card mandate
  // -------------------------------------------------------------------------

  it("drill-out chain card → column → board → window-root flips data-focused at each step", async () => {
    // The verification protocol in card `01KQ5PEHWT...` requires an
    // integration test that walks Escape from a focused card up through
    // its column, then the board zone, then onto the window-root layer.
    // We mimic the kernel's resulting `focus-changed` payload at each
    // step and assert the React tree's `data-focused` attribute follows.
    //
    // Drill-out routing itself lives in the spatial-focus-context tests
    // (the kernel resolves the parent moniker for a given key); what
    // this test pins is "when the kernel does route up, the React tree
    // mirrors the new focus end-to-end across the full chain."
    //
    // Note: the card is a `<FocusScope>` (post-card-`01KQJDYJ4SDKK2G8FTAQ348ZHG`)
    // — every register IPC for cards, columns, and the board lives in
    // `registerScopeArgs()`. Drill-out from a focused atom inside the
    // card lands on the card zone first, then the column zone, then
    // the board chrome zone, then the window-root layer.
    const { container, unmount } = renderBoardWithShell();
    await flushSetup();

    const boardZone = registerScopeArgs().find(
      (a) => a.segment === "board:board-1",
    )!;
    const boardKey = boardZone.fq as FullyQualifiedMoniker;
    const todoColumn = registerScopeArgs().find(
      (a) => a.segment === "column:col-todo",
    )!;
    const todoColumnKey = todoColumn.fq as FullyQualifiedMoniker;
    const t1Card = registerScopeArgs().find((a) => a.segment === "task:t1");
    expect(t1Card, "task:t1 zone should be registered").toBeTruthy();
    const t1CardKey = t1Card!.fq as FullyQualifiedMoniker;

    // Step 1: focus a card (task:t1).
    await fireFocusChanged({ next_fq: t1CardKey });
    await waitFor(() => {
      const cardNode = container.querySelector(
        "[data-segment='task:t1']",
      ) as HTMLElement;
      expect(cardNode).not.toBeNull();
      expect(cardNode.getAttribute("data-focused")).not.toBeNull();
    });

    // Step 2: Escape pops focus to the column.
    await fireFocusChanged({
      prev_fq: t1CardKey,
      next_fq: todoColumnKey,
    });
    await waitFor(() => {
      const cardNode = container.querySelector(
        "[data-segment='task:t1']",
      ) as HTMLElement;
      const columnNode = container.querySelector(
        "[data-segment='column:col-todo']",
      ) as HTMLElement;
      // Card has lost focus; column has gained it.
      expect(cardNode.getAttribute("data-focused")).toBeNull();
      expect(columnNode.getAttribute("data-focused")).not.toBeNull();
    });

    // Step 3: Escape pops focus to the board zone.
    await fireFocusChanged({
      prev_fq: todoColumnKey,
      next_fq: boardKey,
    });
    await waitFor(() => {
      const columnNode = container.querySelector(
        "[data-segment='column:col-todo']",
      ) as HTMLElement;
      const boardNode = container.querySelector(
        "[data-segment='board:board-1']",
      ) as HTMLElement;
      // Column has lost focus; board zone has gained it (data-focused
      // flips even though the visible bar stays suppressed by
      // `showFocus={false}`).
      expect(columnNode.getAttribute("data-focused")).toBeNull();
      expect(boardNode.getAttribute("data-focused")).not.toBeNull();
    });

    // Step 4: Escape pops focus to the window-root layer (no spatial
    // key — the layer's `last_focused` clears).
    await fireFocusChanged({
      prev_fq: boardKey,
      next_fq: null,
    });
    await waitFor(() => {
      const boardNode = container.querySelector(
        "[data-segment='board:board-1']",
      ) as HTMLElement;
      // Board zone has lost focus; nothing in the React tree carries
      // `data-focused` at this point (the window-root layer does not
      // render a DOM node — it is a logical boundary in the registry).
      expect(boardNode.getAttribute("data-focused")).toBeNull();
    });

    unmount();
  });
});
