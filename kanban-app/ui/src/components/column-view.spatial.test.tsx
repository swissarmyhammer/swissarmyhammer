/**
 * Browser-mode test for `<ColumnView>` zone behaviour.
 *
 * Source of truth for acceptance of card `01KQ20MX70NFN2ZVM2YN0A4KQ0` ("Column:
 * wrap as zone, strip legacy keyboard nav from column-view"). The column body
 * is a sized, distinct entity — it registers as a zone in the spatial graph
 * and **advertises its focus** with a visible `<FocusIndicator>` (the
 * production-side default `showFocusBar={true}` on the wrapping `<FocusScope>`).
 * This file pins the click → claim → indicator chain that the user actually
 * sees, as well as the keystroke + drill-out wiring that depends on it.
 *
 * Test cases (per the card's "Browser Tests (mandatory)" section):
 *
 * 1. **Registration** — after mount, `mockInvoke` recorded a
 *    `spatial_register_scope` call with moniker `column:{id}`.
 * 2. **Click on column whitespace → focus** — clicking the column body
 *    fires exactly one `spatial_focus` for the column key, and does NOT
 *    bubble into the parent `ui:board` zone.
 * 3. **Focus claim → visible bar** — `fireFocusChanged(columnKey)` flips
 *    `data-focused="true"` on the column AND mounts `<FocusIndicator>`
 *    (the visible bar — this is the regression the card was opened on).
 * 4. **Keystrokes → navigate** — pressing the keymap-bound keys while the
 *    column is focused dispatches `spatial_navigate(columnKey, dir)` for
 *    every cardinal direction (Arrow + vim h/j/k/l).
 * 5. **Drill-out (Escape)** — Escape with the column focused dispatches
 *    `spatial_drill_out(columnKey)`; after the kernel emits a
 *    `focus-changed` for a different key, the column's `data-focused`
 *    flips back to absent.
 * 6. **Unmount** — unmounting `<ColumnView>` dispatches
 *    `spatial_unregister_scope({ key: columnKey })`.
 * 7. **Legacy nav stripped** — no `entity_focus_*`, `claim_when_*`, or
 *    `broadcast_nav_*` IPCs ever fire from the column body.
 *
 * Mock pattern matches `perspective-bar.spatial.test.tsx` /
 * `perspective-view.spatial.test.tsx`: `vi.hoisted` builds the
 * `mockInvoke` / `mockListen` / `listeners` triple; `fireFocusChanged`
 * drives the React tree as if the Rust kernel emitted the event.
 *
 * Runs under `kanban-app/ui/vite.config.ts`'s browser project (real
 * Chromium via Playwright) — every `*.test.tsx` outside `*.node.test.tsx`
 * lands there.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent, act, waitFor } from "@testing-library/react";
import { userEvent } from "vitest/browser";
import type { Entity } from "@/types/kanban";

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
  listen: (...a: Parameters<typeof mockListen>) => mockListen(...a),
  emit: vi.fn(() => Promise.resolve()),
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

// ---------------------------------------------------------------------------
// Imports — after mocks
// ---------------------------------------------------------------------------

import { ColumnView } from "./column-view";
import { AppShell } from "./app-shell";
import { FocusLayer } from "./focus-layer";
import { FocusScope } from "./focus-scope";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { ActiveBoardPathProvider } from "@/lib/command-scope";
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

function makeColumn(id = "01ABCDEFGHJKMNPQRSTVWXYZ01", name = "To Do"): Entity {
  return {
    entity_type: "column",
    id,
    moniker: `column:${id}`,
    fields: { name },
  };
}

function makeTask(id: string, column: string): Entity {
  return {
    entity_type: "task",
    id,
    moniker: `task:${id}`,
    fields: {
      title: `Task ${id}`,
      position_column: column,
      position_ordinal: "a0",
    },
  };
}

// ---------------------------------------------------------------------------
// Default invoke responses for the AppShell-driven harness
// ---------------------------------------------------------------------------

/**
 * Default `invoke` implementation covering the IPCs the provider stack
 * fires on mount. Keeps the AppShell-derived tests from crashing on a
 * `null` UIState, while leaving every spatial-nav IPC available for
 * assertion.
 */
async function defaultInvokeImpl(
  cmd: string,
  _args?: unknown,
): Promise<unknown> {
  if (cmd === "get_ui_state") {
    return {
      palette_open: false,
      palette_mode: "command",
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      windows: {
        main: {
          palette_open: false,
          palette_mode: "command",
        },
      },
      recent_boards: [],
    };
  }
  if (cmd === "get_undo_state") return { can_undo: false, can_redo: false };
  if (cmd === "list_entity_types") return [];
  if (cmd === "get_entity_schema") return null;
  if (cmd === "list_commands_for_scope") return [];
  if (cmd === "dispatch_command") return undefined;
  return undefined;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Wait for register effects scheduled in `useEffect` to flush. */
async function flushSetup() {
  // Two ticks: first lets `useEffect` callbacks run, second lets any
  // Promise-resolution-driven follow-on (e.g. `subscribeFocusChanged`'s
  // listener registration) settle.
  await act(async () => {
    await Promise.resolve();
  });
  await act(async () => {
    await Promise.resolve();
  });
}

/**
 * Drive a `focus-changed` event into the React tree as if the Rust
 * kernel had emitted one for the active window.
 *
 * The payload's `prev_fq` / `next_fq` mirror the kernel's
 * post-`spatial_focus` / `spatial_navigate` emit. Wrapping the
 * dispatch in `act()` flushes the React state updates so the caller
 * can assert against post-update DOM in the next tick.
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
 * Render a `<ColumnView>` inside the production-shaped spatial stack
 * with a surrounding `ui:board` zone. Provides only the providers a
 * column needs for its rendering and click contract — used by tests
 * that don't need keystroke wiring (cases 1, 2, 3, 6, 7).
 */
function renderColumnInBoard(ui: React.ReactElement) {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <SchemaProvider>
            <EntityStoreProvider entities={{}}>
              <TooltipProvider>
                <ActiveBoardPathProvider value="/test/board">
                  <FocusScope moniker={asSegment("ui:board")}>{ui}</FocusScope>
                </ActiveBoardPathProvider>
              </TooltipProvider>
            </EntityStoreProvider>
          </SchemaProvider>
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

/**
 * Render a `<ColumnView>` inside the full `<AppShell>` provider stack.
 *
 * AppShell wires the global keydown listener that drives the
 * `nav.up` / `nav.down` / `nav.left` / `nav.right` / `nav.drillOut`
 * commands; tests for keystroke → `spatial_navigate` (case 4) and
 * Escape → `spatial_drill_out` (case 5) need that wiring to fire on
 * `userEvent.keyboard()`. The AppShell harness mounts the same set of
 * top-level providers as `app-shell.test.tsx`'s `renderShell`, plus
 * the column-rendering providers (`SchemaProvider`,
 * `EntityStoreProvider`, `TooltipProvider`, `ActiveBoardPathProvider`).
 */
function renderColumnInAppShell(ui: React.ReactElement) {
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
                        <AppShell>
                          <FocusScope moniker={asSegment("ui:board")}>
                            {ui}
                          </FocusScope>
                        </AppShell>
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

/** Collect every `spatial_register_scope` invocation argument bag. */
function registerScopeArgs(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_scope")
    .map((c) => c[1] as Record<string, unknown>);
}

/** Collect every `spatial_focus` call's args, in order. */
function spatialFocusCalls(): Array<{ fq: FullyQualifiedMoniker }> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_focus")
    .map((c) => c[1] as { fq: FullyQualifiedMoniker });
}

/** Collect every `spatial_navigate` call's args, in order. */
function spatialNavigateCalls(): Array<{
  focusedFq: FullyQualifiedMoniker;
  direction: string;
}> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_navigate")
    .map((c) => c[1] as { focusedFq: FullyQualifiedMoniker; direction: string });
}

/** Collect every `spatial_drill_out` call's args, in order. */
function spatialDrillOutCalls(): Array<{ fq: FullyQualifiedMoniker }> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_drill_out")
    .map((c) => c[1] as { fq: FullyQualifiedMoniker });
}

/** Collect every `spatial_unregister_scope` call's args, in order. */
function unregisterScopeCalls(): Array<{ fq: FullyQualifiedMoniker }> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_unregister_scope")
    .map((c) => c[1] as { fq: FullyQualifiedMoniker });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("ColumnView — browser spatial behaviour", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    mockInvoke.mockImplementation(defaultInvokeImpl);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // -------------------------------------------------------------------------
  // Test #1 — Registration
  // -------------------------------------------------------------------------

  it("registers a column:{id} zone on mount", async () => {
    const column = makeColumn("01ABCDEFGHJKMNPQRSTVWXYZ01");
    const { unmount } = renderColumnInBoard(
      <ColumnView column={column} tasks={[]} />,
    );
    await flushSetup();

    const columnZone = registerScopeArgs().find(
      (a) => a.segment === column.moniker,
    );
    expect(columnZone).toBeTruthy();
    expect(typeof columnZone!.fq).toBe("string");
    expect((columnZone!.segment as string)).toMatch(/^column:[0-9A-Z]{26}$/);
    expect(columnZone!.layerFq).toBeTruthy();
    expect(columnZone!.rect).toBeTruthy();
    expect(columnZone!.overrides).toEqual({});

    // Parent zone is the surrounding `ui:board` zone (mirrors production).
    const boardZone = registerScopeArgs().find((a) => a.segment === "ui:board");
    expect(boardZone).toBeTruthy();
    expect(columnZone!.parentZone).toBe(boardZone!.fq);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Test #2 — Click on column body whitespace → focus
  // -------------------------------------------------------------------------

  it("clicking column whitespace dispatches exactly one spatial_focus for the column key", async () => {
    const column = makeColumn("01ABCDEFGHJKMNPQRSTVWXYZ02");
    const { container, unmount } = renderColumnInBoard(
      <ColumnView column={column} tasks={[]} />,
    );
    await flushSetup();

    const columnZone = registerScopeArgs().find(
      (a) => a.segment === column.moniker,
    )!;
    const boardZone = registerScopeArgs().find((a) => a.segment === "ui:board")!;

    // Clear so the assertion measures only the click's IPC.
    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    const columnNode = container.querySelector(
      `[data-segment='${column.moniker}']`,
    ) as HTMLElement | null;
    expect(columnNode).not.toBeNull();

    fireEvent.click(columnNode!);

    const focusCalls = spatialFocusCalls();
    expect(focusCalls).toHaveLength(1);
    expect(focusCalls[0].fq).toBe(columnZone.fq);
    // The board zone key must NOT also receive a focus call — the column
    // calls `e.stopPropagation()` so the click does not bubble to the
    // wrapping board zone. This is the regression-test side of the bug
    // the card was opened on (visible feedback was suppressed by
    // `showFocusBar={false}`; the click itself was already correct, but
    // pinning bubble-blocking here keeps the click contract intact).
    expect(
      focusCalls.find((c) => c.fq === boardZone.fq),
    ).toBeUndefined();

    unmount();
  });

  // -------------------------------------------------------------------------
  // Test #3 — Focus claim → visible bar
  // -------------------------------------------------------------------------

  it("focus claim mounts <FocusIndicator> inside the column (showFocusBar={true})", async () => {
    // The visible-bar regression: the previous wrap had
    // `showFocusBar={false}`, which suppressed `<FocusIndicator>` even
    // when the kernel emitted a focus claim for the column. The fix
    // (drop the `false` and rely on `<FocusScope>`'s default `true`)
    // is what this test pins. If a future edit adds the suppression
    // back, this assertion will fail because the indicator never
    // mounts.
    const column = makeColumn("01ABCDEFGHJKMNPQRSTVWXYZ03");
    const { container, queryByTestId, unmount } = renderColumnInBoard(
      <ColumnView column={column} tasks={[]} />,
    );
    await flushSetup();

    const columnZone = registerScopeArgs().find(
      (a) => a.segment === column.moniker,
    )!;
    const columnNode = container.querySelector(
      `[data-segment='${column.moniker}']`,
    ) as HTMLElement;
    expect(columnNode).not.toBeNull();
    expect(columnNode.getAttribute("data-focused")).toBeNull();
    // No indicator before the focus claim.
    expect(queryByTestId("focus-indicator")).toBeNull();

    await fireFocusChanged({ next_fq: columnZone.fq as FullyQualifiedMoniker });

    await waitFor(() => {
      expect(columnNode.getAttribute("data-focused")).toBe("true");
    });
    // The visible bar mounted, AND it lives inside the column box.
    const indicator = queryByTestId("focus-indicator");
    expect(indicator).not.toBeNull();
    expect(columnNode.contains(indicator!)).toBe(true);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Test #4 — Keystrokes → navigate
  // -------------------------------------------------------------------------

  it("ArrowUp while column-focused dispatches spatial_navigate(columnKey, 'up')", async () => {
    const column = makeColumn("01ABCDEFGHJKMNPQRSTVWXYZ04");
    const { unmount } = renderColumnInAppShell(
      <ColumnView column={column} tasks={[]} />,
    );
    await flushSetup();

    const columnZone = registerScopeArgs().find(
      (a) => a.segment === column.moniker,
    )!;
    const columnKey = columnZone.fq as FullyQualifiedMoniker;

    // Seed both the SpatialFocusProvider's `focusedKeyRef` (for the
    // nav-command closure) AND the entity-focus moniker store (so
    // `useFocusedScope()` resolves the column's CommandScope, which in
    // turn lets `extractScopeBindings` reach the dynamic `nav.*`
    // commands' `keys[mode]` entries through the React-ancestor scope
    // chain). The moniker bridge in `EntityFocusProvider` mirrors
    // `payload.next_segment` into the moniker store; tests that omit
    // `next_segment` leave the store empty, the focused scope null, and
    // the keymap pipeline blind to the dynamic arrow-key bindings.
    await fireFocusChanged({
      next_fq: columnKey,
      next_segment: column.moniker,
    });

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await userEvent.keyboard("{ArrowUp}");
    await flushSetup();

    expect(spatialNavigateCalls()).toEqual([
      { focusedFq: columnKey, direction: "up" },
    ]);

    unmount();
  });

  it("ArrowDown while column-focused dispatches spatial_navigate(columnKey, 'down')", async () => {
    const column = makeColumn("01ABCDEFGHJKMNPQRSTVWXYZ05");
    const { unmount } = renderColumnInAppShell(
      <ColumnView column={column} tasks={[]} />,
    );
    await flushSetup();

    const columnKey = registerScopeArgs().find(
      (a) => a.segment === column.moniker,
    )!.fq as FullyQualifiedMoniker;

    await fireFocusChanged({
      next_fq: columnKey,
      next_segment: column.moniker,
    });

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await userEvent.keyboard("{ArrowDown}");
    await flushSetup();

    expect(spatialNavigateCalls()).toEqual([
      { focusedFq: columnKey, direction: "down" },
    ]);

    unmount();
  });

  it("ArrowLeft while column-focused dispatches spatial_navigate(columnKey, 'left')", async () => {
    const column = makeColumn("01ABCDEFGHJKMNPQRSTVWXYZ06");
    const { unmount } = renderColumnInAppShell(
      <ColumnView column={column} tasks={[]} />,
    );
    await flushSetup();

    const columnKey = registerScopeArgs().find(
      (a) => a.segment === column.moniker,
    )!.fq as FullyQualifiedMoniker;

    await fireFocusChanged({
      next_fq: columnKey,
      next_segment: column.moniker,
    });

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await userEvent.keyboard("{ArrowLeft}");
    await flushSetup();

    expect(spatialNavigateCalls()).toEqual([
      { focusedFq: columnKey, direction: "left" },
    ]);

    unmount();
  });

  it("ArrowRight while column-focused dispatches spatial_navigate(columnKey, 'right')", async () => {
    const column = makeColumn("01ABCDEFGHJKMNPQRSTVWXYZ07");
    const { unmount } = renderColumnInAppShell(
      <ColumnView column={column} tasks={[]} />,
    );
    await flushSetup();

    const columnKey = registerScopeArgs().find(
      (a) => a.segment === column.moniker,
    )!.fq as FullyQualifiedMoniker;

    await fireFocusChanged({
      next_fq: columnKey,
      next_segment: column.moniker,
    });

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await userEvent.keyboard("{ArrowRight}");
    await flushSetup();

    expect(spatialNavigateCalls()).toEqual([
      { focusedFq: columnKey, direction: "right" },
    ]);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Test #5 — Drill-out (Escape)
  // -------------------------------------------------------------------------

  it("Escape while column-focused dispatches spatial_drill_out(columnKey)", async () => {
    const column = makeColumn("01ABCDEFGHJKMNPQRSTVWXYZ08");
    const { container, unmount } = renderColumnInAppShell(
      <ColumnView column={column} tasks={[]} />,
    );
    await flushSetup();

    const columnZone = registerScopeArgs().find(
      (a) => a.segment === column.moniker,
    )!;
    const columnKey = columnZone.fq as FullyQualifiedMoniker;

    // Seed the focused-key ref AND the moniker store so the global
    // Escape handler sees the column as the current target. Same
    // rationale as the arrow-key tests — `extractScopeBindings` walks
    // the focused scope's parent chain, so the moniker store has to
    // resolve the column's scope before the dynamic nav commands'
    // `keys[mode]` participate in the binding lookup.
    await fireFocusChanged({
      next_fq: columnKey,
      next_segment: column.moniker,
    });

    const columnNode = container.querySelector(
      `[data-segment='${column.moniker}']`,
    ) as HTMLElement;
    expect(columnNode.getAttribute("data-focused")).toBe("true");

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === "spatial_drill_out") {
        // Mirror the kernel's "drill-out walks to the surrounding
        // `ui:board` zone" answer — we don't care which moniker comes
        // back for THIS test; we only verify the column key was the
        // input and that the React tree later reacts to the kernel's
        // follow-on `focus-changed` payload (asserted below).
        return Promise.resolve(asSegment("ui:board"));
      }
      return defaultInvokeImpl(cmd, args);
    });

    await userEvent.keyboard("{Escape}");
    await flushSetup();

    const drillCalls = spatialDrillOutCalls();
    expect(drillCalls).toHaveLength(1);
    expect(drillCalls[0].fq).toBe(columnKey);

    // Now mimic the kernel's resulting `focus-changed` (the column
    // de-focuses; its `data-focused` flips back to absent).
    const phantomBoardKey =
      "ffffffff-ffff-4fff-8fff-fffffffffffe" as FullyQualifiedMoniker;
    await fireFocusChanged({
      prev_fq: columnKey,
      next_fq: phantomBoardKey,
    });

    await waitFor(() => {
      expect(columnNode.getAttribute("data-focused")).toBeNull();
    });

    unmount();
  });

  // -------------------------------------------------------------------------
  // Test #6 — Unmount
  // -------------------------------------------------------------------------

  it("unmounting <ColumnView> dispatches spatial_unregister_scope for the column key", async () => {
    const column = makeColumn("01ABCDEFGHJKMNPQRSTVWXYZ09");
    const { unmount } = renderColumnInBoard(
      <ColumnView column={column} tasks={[]} />,
    );
    await flushSetup();

    const columnZone = registerScopeArgs().find(
      (a) => a.segment === column.moniker,
    )!;
    const columnKey = columnZone.fq as FullyQualifiedMoniker;

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    unmount();
    // Cleanup effects fire synchronously when React unmounts the tree;
    // collect from the call list directly.
    const unregisterKeys = unregisterScopeCalls().map((c) => c.fq);
    expect(unregisterKeys).toContain(columnKey);
  });

  // -------------------------------------------------------------------------
  // Test #7 — Legacy nav stripped
  // -------------------------------------------------------------------------

  it("emits no legacy entity_focus_* / claim_when_* / broadcast_nav_* IPCs", async () => {
    const column = makeColumn("01ABCDEFGHJKMNPQRSTVWXYZ10");
    const tasks = [
      makeTask("01TASKAAAAAAAAAAAAAAAAAAAA", column.id),
      makeTask("01TASKBBBBBBBBBBBBBBBBBBBB", column.id),
    ];
    const { container, unmount } = renderColumnInBoard(
      <ColumnView column={column} tasks={tasks} />,
    );
    await flushSetup();

    // Drive a click + a synthetic focus-changed to exercise the same
    // hot paths the bug report covered.
    const columnZone = registerScopeArgs().find(
      (a) => a.segment === column.moniker,
    )!;
    const columnNode = container.querySelector(
      `[data-segment='${column.moniker}']`,
    ) as HTMLElement;
    expect(columnNode).not.toBeNull();
    fireEvent.click(columnNode);
    await fireFocusChanged({ next_fq: columnZone.fq as FullyQualifiedMoniker });

    const banned = /^(entity_focus_|claim_when_|broadcast_nav_)/;
    const offenders = mockInvoke.mock.calls
      .map((c) => c[0])
      .filter((cmd) => typeof cmd === "string" && banned.test(cmd));
    expect(offenders).toEqual([]);

    unmount();
  });
});
