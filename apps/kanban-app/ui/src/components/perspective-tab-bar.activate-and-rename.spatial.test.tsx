/**
 * Browser-mode test for the spatial-focus keyboard contract on
 * `<PerspectiveTabBar>` tabs: Enter ACTIVATES, F2 renames.
 *
 * Source of truth for acceptance of card `01KTYQY0ZB62KHN6BPK3FBMBD7`
 * ("Perspectives can't be SELECTED — Enter arms rename instead, click
 * doesn't activate"). Supersedes the previous
 * `perspective-tab-bar.enter-rename.spatial.test.tsx`, which pinned the
 * old Enter→rename carrier (cards 01KQ7GE3KY91X2YR6BX5AY40VK /
 * 01KQAXPRTCNH8ARTYJJEBTYWW0).
 *
 * The contract (Enter-drill idiom refined by card
 * 01KV0MBDBW06NRXCWVZBJ0445S):
 *
 * - **Enter = the tab/drill idiom.** A focused tab registers a positional
 *   `nav.drillIn` shadow `CommandDef` (the documented scope-local-execute
 *   pattern — same as the jump overlay's `app.dismiss` shadow): the global
 *   Enter binding resolves to `nav.drillIn`, and while the tab is the
 *   focused scope the shadow's execute branches on active-ness — an
 *   INACTIVE tab dispatches `perspective.switch` (select/activate); the
 *   ALREADY-ACTIVE tab drills into the caption editor by arming the
 *   existing inline rename (no re-switch, the same editor F2 uses).
 * - **F2 = rename.** Rename is a deliberate, separate gesture — F2 is the
 *   platform-wide rename idiom (and the per-keymap rename key on all three
 *   modes). The per-tab `app.entity.startRename` `CommandDef` carries the
 *   F2 keys and keeps the activate-then-rename execute: on an inactive tab
 *   it dispatches `perspective.switch` first, then arms the inline editor
 *   on the focused tab by explicit id. Double-click stays as the pointer
 *   rename gesture, and the catalogue registration
 *   (`app-shell-commands/commands/ui.ts`) mirrors the F2 keys +
 *   `context_menu` for the right-click row.
 *
 * Test cases:
 *
 * 1. **Enter on the already-active tab drills into rename** — arms the
 *    inline caption editor and does NOT re-dispatch `perspective.switch`.
 * 2. **Enter on a focused inactive tab activates that tab** — dispatches
 *    `perspective.switch` with the focused tab's id; no rename editor.
 * 3. **Enter outside perspective scope still drills** — focusing a non-
 *    perspective leaf and pressing Enter dispatches `nav.drillIn` to the
 *    backend. Proves the shadow is scope-local.
 * 4. **Vim / emacs Enter on the already-active tab** — same as case 1 per
 *    keymap mode (arms rename, no re-switch).
 * 5. **F2 mounts the inline rename editor** (cua / vim / emacs) on the
 *    focused active tab.
 * 6. **F2 on a focused inactive tab activates then renames** — dispatches
 *    `perspective.switch` AND mounts the editor on that tab.
 * 7. **Commit path** — after F2 mounts the editor, typing + Enter
 *    dispatches `perspective.rename` with `{ id, new_name }`.
 * 8. **Escape preserves existing policy** — cua/emacs cancel, vim commits,
 *    per the existing `useInlineRenamePolicy` contract.
 *
 * Mock pattern matches `column-view.spatial.test.tsx`: `vi.hoisted` builds
 * the `mockInvoke` / `mockListen` / `listeners` triple; `fireFocusChanged`
 * drives the React tree as if the Rust kernel emitted a `focus-changed`
 * event for the captured spatial key.
 *
 * Runs under `kanban-app/ui/vite.config.ts`'s browser project (real
 * Chromium via Playwright).
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { act, waitFor } from "@testing-library/react";
import { renderInAct, pressKeyInAct } from "@/test/act-render";
import type { ReactElement } from "react";
import { TooltipProvider } from "@/components/ui/tooltip";

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
// Perspective + view + UI mocks — the tab bar reads from these contexts.
// ---------------------------------------------------------------------------

type MockPerspective = {
  id: string;
  name: string;
  view: string;
  filter?: string;
  group?: string;
};

let mockPerspectivesValue = {
  perspectives: [] as MockPerspective[],
  activePerspective: null as MockPerspective | null,
  setActivePerspectiveId: vi.fn(),
  refresh: vi.fn(() => Promise.resolve()),
};

vi.mock("@/lib/perspective-context", () => ({
  usePerspectives: () => mockPerspectivesValue,
  PerspectiveProvider: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
}));

let mockViewsValue = {
  views: [{ id: "board-1", name: "Board", kind: "board", icon: "kanban" }],
  activeView: { id: "board-1", name: "Board", kind: "board", icon: "kanban" },
  setActiveViewId: vi.fn(),
  refresh: vi.fn(() => Promise.resolve()),
};

vi.mock("@/lib/views-context", () => ({
  useViews: () => mockViewsValue,
}));

vi.mock("@/lib/context-menu", () => ({
  useContextMenu: () => vi.fn(),
}));

vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({ getEntities: () => [] }),
}));

vi.mock("@/components/window-container", () => ({
  useBoardData: () => ({ virtualTagMeta: [] }),
}));

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => ({ entity: { name: "task", fields: [] }, fields: [] }),
    getFieldDef: () => undefined,
    mentionableTypes: [],
    loading: false,
  }),
}));

// Mutable keymap so individual tests can switch between cua / vim / emacs
// without remounting the entire mock stack.
let mockKeymapMode: "cua" | "vim" | "emacs" = "cua";

const mockUIState = () => ({
  keymap_mode: mockKeymapMode,
  scope_chain: [],
  open_boards: [],
  has_clipboard: false,
  clipboard_entity_type: null,
  windows: { main: { palette_open: false, palette_mode: "command" } },
  recent_boards: [],
});

vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => mockUIState(),
  useUIStateLoading: () => ({ state: mockUIState(), loading: false }),
  UIStateProvider: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
}));

// ---------------------------------------------------------------------------
// Imports — after mocks
// ---------------------------------------------------------------------------

import { PerspectiveTabBar } from "./perspective-tab-bar";
import { AppShell } from "./app-shell";
import { commandToolCall } from "@/test/mock-command-list";
import { FocusLayer } from "./focus-layer";
import { FocusScope } from "./focus-scope";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { AppModeProvider } from "@/lib/app-mode-context";
import { UndoProvider } from "@/lib/undo-context";
import { ActiveBoardPathProvider } from "@/lib/command-scope";
import {
  asSegment,
  type FocusChangedPayload,
  type FullyQualifiedMoniker,
  type WindowLabel,
} from "@/types/spatial";

// ---------------------------------------------------------------------------
// Default invoke responses — keep the AppShell-driven harness alive.
// ---------------------------------------------------------------------------

/**
 * Default `invoke` implementation covering the IPCs the provider stack
 * fires on mount. Mirrors `column-view.spatial.test.tsx`'s impl so the
 * AppShell-derived harness lights up the same set of providers.
 */
async function defaultInvokeImpl(
  cmd: string,
  _args?: unknown,
): Promise<unknown> {
  // The global keybinding layer (Enter → `nav.drillIn`, …) is sourced from
  // the metadata-driven Command registry via `useCommandList`, which fetches
  // through the `command_tool_call` bridge's `list command` op. Synthesize
  // that registry from `BINDING_TABLES` so global bindings resolve — Enter
  // must resolve to a REAL global `nav.drillIn` whose execution the focused
  // tab's positional shadow then intercepts.
  if (cmd === "command_tool_call") {
    return commandToolCall(_args);
  }
  if (cmd === "get_ui_state") {
    return {
      palette_open: false,
      palette_mode: "command",
      keymap_mode: mockKeymapMode,
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
 * Drive a `focus-changed` event into the React tree as if the Rust kernel
 * had emitted one for the active window.
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
  const handlers = listeners.get("notifications/focus/changed") ?? [];
  await act(async () => {
    for (const handler of handlers) handler({ payload });
    await Promise.resolve();
  });
}

/**
 * Render the bar inside the full `<AppShell>` provider stack.
 *
 * AppShell wires the global keydown listener that drives the
 * `nav.drillIn` (Enter) and per-tab `app.entity.startRename` (F2)
 * commands, so keystroke tests need that wiring to fire on
 * `userEvent.keyboard()`.
 */
async function renderInAppShell(extraChildren?: ReactElement) {
  return await renderInAct(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <AppModeProvider>
            <UndoProvider>
              <TooltipProvider delayDuration={100}>
                <ActiveBoardPathProvider value="/test/board">
                  <AppShell>
                    <PerspectiveTabBar />
                    {extraChildren}
                  </AppShell>
                </ActiveBoardPathProvider>
              </TooltipProvider>
            </UndoProvider>
          </AppModeProvider>
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

/** Capture every `spatial_register_scope` invocation argument bag. */
function registerScopeArgs(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_scope")
    .map((c) => c[1] as Record<string, unknown>);
}

/** Capture every `dispatch_command` call's args. */
function dispatchCalls(): Array<{
  cmd: string;
  args?: Record<string, unknown>;
}> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "dispatch_command")
    .map((c) => c[1] as { cmd: string; args?: Record<string, unknown> });
}

/** True when any dispatch_command call had cmd === `target`. */
function dispatchedCommand(target: string): boolean {
  return dispatchCalls().some((d) => d.cmd === target);
}

/** Find the dispatch_command call for a given cmd id. */
function findDispatch(target: string):
  | {
      cmd: string;
      args?: Record<string, unknown>;
    }
  | undefined {
  return dispatchCalls().find((d) => d.cmd === target);
}

/** True when any registered scope has the given moniker. */
function findScopeKey(moniker: string): FullyQualifiedMoniker | undefined {
  const scope = registerScopeArgs().find((a) => a.segment === moniker);
  return scope?.fq as FullyQualifiedMoniker | undefined;
}

/** Spatially focus the scope with the given moniker and wait for the claim. */
async function focusTab(container: HTMLElement, moniker: string) {
  const key = findScopeKey(moniker);
  expect(key).toBeTruthy();
  await fireFocusChanged({
    next_fq: key!,
    next_segment: asSegment(moniker),
  });
  await waitFor(() => {
    const focusedTab = container.querySelector(`[data-segment='${moniker}']`);
    expect(focusedTab?.getAttribute("data-focused")).toBe("true");
  });
}

/** Assert no inline rename editor is mounted inside any perspective tab. */
function expectNoRenameEditor(container: HTMLElement) {
  expect(
    container.querySelectorAll("[data-segment^='perspective_tab:'] .cm-editor"),
  ).toHaveLength(0);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("PerspectiveTabBar — Enter activates the focused tab; F2 renames", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    mockInvoke.mockImplementation(defaultInvokeImpl);
    mockKeymapMode = "cua";
    mockPerspectivesValue = {
      perspectives: [
        { id: "p1", name: "Sprint", view: "board" },
        { id: "p2", name: "Backlog", view: "board" },
      ],
      activePerspective: { id: "p1", name: "Sprint", view: "board" },
      setActivePerspectiveId: vi.fn(),
      refresh: vi.fn(() => Promise.resolve()),
    };
    mockViewsValue = {
      views: [{ id: "board-1", name: "Board", kind: "board", icon: "kanban" }],
      activeView: {
        id: "board-1",
        name: "Board",
        kind: "board",
        icon: "kanban",
      },
      setActiveViewId: vi.fn(),
      refresh: vi.fn(() => Promise.resolve()),
    };
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // -------------------------------------------------------------------------
  // Test #1 — Enter on the ALREADY-ACTIVE tab arms inline rename (drill-in),
  // and does NOT re-dispatch perspective.switch (the tab/drill idiom).
  // -------------------------------------------------------------------------

  it("Enter on the focused already-active perspective tab arms inline rename and does NOT re-dispatch perspective.switch", async () => {
    const { container, unmount } = await renderInAppShell();
    await flushSetup();

    await focusTab(container, "perspective_tab:p1");

    // Reset invoke calls so the assertion measures only Enter's IPC.
    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await pressKeyInAct("{Enter}");
    await flushSetup();

    // Drill idiom: Enter on the tab that is ALREADY the active perspective
    // drills into the caption editor — the same inline rename machinery F2
    // and double-click use — instead of re-selecting (a no-op switch).
    await waitFor(() => {
      const renameEditor = container.querySelector(
        "[data-segment='perspective_tab:p1'] .cm-editor",
      );
      expect(renameEditor).not.toBeNull();
    });

    // The already-active tab must NOT re-dispatch the switch command.
    expect(dispatchedCommand("perspective.switch")).toBe(false);

    // And the shadow swallowed the drill — no backend nav.drillIn dispatch.
    expect(dispatchedCommand("nav.drillIn")).toBe(false);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Test #2 — Enter on a focused INACTIVE tab activates that tab
  // -------------------------------------------------------------------------

  it("Enter on a focused inactive perspective tab dispatches perspective.switch for that tab", async () => {
    const { container, unmount } = await renderInAppShell();
    await flushSetup();

    // Active perspective is `p1`; spatially focus the inactive `p2` tab.
    await focusTab(container, "perspective_tab:p2");

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await pressKeyInAct("{Enter}");
    await flushSetup();

    const switchDispatch = findDispatch("perspective.switch");
    expect(switchDispatch).toBeTruthy();
    expect(switchDispatch!.args).toEqual(
      expect.objectContaining({ perspective_id: "p2" }),
    );

    expectNoRenameEditor(container);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Test #2b — PRODUCTION drill path: a SECOND Enter on the tab the user just
  // selected drills into the caption editor even before the `perspective.switch`
  // UI-state event has propagated `activePerspectiveId` back to the prop.
  //
  // This reproduces the real-app failure card 01KV5JC141Z0TZQSZTW4KZ12PM is
  // about. The `activePerspective` mock deliberately STAYS on `p1` across both
  // Enter presses — modeling the async lag between dispatching
  // `perspective.switch` and the per-window `active_perspective_id` UI-state
  // event landing (perspective-context.tsx derives `activePerspective` from
  // `uiState.windows[main].active_perspective_id`, so the prop trails the
  // dispatch by a round-trip). Test #1 never exercises this: it starts with
  // `activePerspective === p1` AND focuses p1, so `isActive` is fresh.
  //
  // Sequence:
  //   1. Focus the INACTIVE tab `p2` (active is `p1`).
  //   2. Enter → select: dispatch `perspective.switch({ perspective_id: p2 })`.
  //   3. Enter AGAIN on the same focused tab → the drill idiom must arm the
  //      inline caption editor on `p2` and must NOT re-dispatch the switch,
  //      even though the stale `activePerspectiveId` prop still reads `p1`.
  // -------------------------------------------------------------------------

  it("a second Enter on the just-selected tab drills into the caption editor before the switch UI-state event propagates (stale activePerspectiveId)", async () => {
    const { container, unmount } = await renderInAppShell();
    await flushSetup();

    // Active perspective is `p1`; spatially focus the inactive `p2` tab.
    await focusTab(container, "perspective_tab:p2");

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    // First Enter — selects p2 (dispatches the switch). The mock's
    // `activePerspective` is NOT advanced, mirroring the production lag.
    await pressKeyInAct("{Enter}");
    await flushSetup();

    const firstSwitch = findDispatch("perspective.switch");
    expect(firstSwitch).toBeTruthy();
    expect(firstSwitch!.args).toEqual(
      expect.objectContaining({ perspective_id: "p2" }),
    );
    // First Enter selects, it must not arm the editor.
    expectNoRenameEditor(container);

    // Reset the dispatch spy so the second Enter's IPC is measured alone.
    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    // Second Enter on the still-focused, just-selected tab — drill in.
    await pressKeyInAct("{Enter}");
    await flushSetup();

    // The caption editor for p2 must mount (drill into the name editor).
    await waitFor(() => {
      const renameEditor = container.querySelector(
        "[data-segment='perspective_tab:p2'] .cm-editor",
      );
      expect(renameEditor).not.toBeNull();
    });

    // And the second Enter must NOT re-dispatch the switch — re-selecting the
    // tab you just selected is a no-op; Enter drills.
    expect(dispatchedCommand("perspective.switch")).toBe(false);
    expect(dispatchedCommand("nav.drillIn")).toBe(false);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Test #3 — Enter outside the perspective scope still drills in
  // -------------------------------------------------------------------------

  it("Enter on a non-perspective focused leaf still dispatches nav.drillIn to the backend", async () => {
    // Mount a non-perspective FocusScope (`task:01ABC`) alongside the
    // perspective bar so the test can drive focus to a leaf that has
    // nothing to do with the perspective scope chain. Enter while that
    // leaf is focused MUST hit the global `nav.drillIn` binding — proving
    // the tab's nav.drillIn shadow is scope-local and does not leak to
    // other focus contexts.
    const { container, unmount } = await renderInAppShell(
      <FocusScope moniker={asSegment("task:01ABC")} commands={[]}>
        <div data-testid="non-perspective-leaf">leaf</div>
      </FocusScope>,
    );
    await flushSetup();

    const taskKey = registerScopeArgs().find((a) => a.segment === "task:01ABC")
      ?.fq as FullyQualifiedMoniker | undefined;
    expect(taskKey).toBeTruthy();

    await fireFocusChanged({
      next_fq: taskKey!,
      next_segment: asSegment("task:01ABC"),
    });

    await waitFor(() => {
      const focused = container.querySelector("[data-segment='task:01ABC']");
      expect(focused?.getAttribute("data-focused")).toBe("true");
    });

    // Reset invoke calls so the assertion measures only Enter's IPC.
    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await pressKeyInAct("{Enter}");
    await flushSetup();

    // Enter routes the `nav.drillIn` plugin command id to the backend —
    // drill executes host-side in the plugin runtime, which resolves the
    // focused FQ itself. The webview sends no fq.
    const drillDispatches = dispatchCalls().filter(
      (d) => d.cmd === "nav.drillIn",
    );
    expect(drillDispatches).toHaveLength(1);

    // No legacy client-side drill IPC — that mechanic moved host-side.
    expect(
      mockInvoke.mock.calls.filter((c) => c[0] === "spatial_drill_in"),
    ).toHaveLength(0);

    // No activation either — the tab shadow must not fire off-tab.
    expect(dispatchedCommand("perspective.switch")).toBe(false);

    // No rename editor mounted in any perspective tab.
    expectNoRenameEditor(container);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Test #4 — Vim / emacs Enter on the already-active tab arms inline rename
  // -------------------------------------------------------------------------

  it("vim: Enter on the focused already-active perspective tab arms inline rename, no re-switch", async () => {
    mockKeymapMode = "vim";
    const { container, unmount } = await renderInAppShell();
    await flushSetup();

    await focusTab(container, "perspective_tab:p1");

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await pressKeyInAct("{Enter}");
    await flushSetup();

    await waitFor(() => {
      const renameEditor = container.querySelector(
        "[data-segment='perspective_tab:p1'] .cm-editor",
      );
      expect(renameEditor).not.toBeNull();
    });
    expect(dispatchedCommand("perspective.switch")).toBe(false);

    unmount();
  });

  it("emacs: Enter on the focused already-active perspective tab arms inline rename, no re-switch", async () => {
    mockKeymapMode = "emacs";
    const { container, unmount } = await renderInAppShell();
    await flushSetup();

    await focusTab(container, "perspective_tab:p1");

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await pressKeyInAct("{Enter}");
    await flushSetup();

    await waitFor(() => {
      const renameEditor = container.querySelector(
        "[data-segment='perspective_tab:p1'] .cm-editor",
      );
      expect(renameEditor).not.toBeNull();
    });
    expect(dispatchedCommand("perspective.switch")).toBe(false);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Test #5 — F2 mounts the inline rename editor (cua / vim / emacs)
  // -------------------------------------------------------------------------

  it("F2 on the focused active perspective tab mounts the inline rename editor", async () => {
    const { container, unmount } = await renderInAppShell();
    await flushSetup();

    expectNoRenameEditor(container);

    await focusTab(container, "perspective_tab:p1");

    await pressKeyInAct("{F2}");
    await flushSetup();

    await waitFor(() => {
      const renameEditor = container.querySelector(
        "[data-segment='perspective_tab:p1'] .cm-editor",
      );
      expect(renameEditor).not.toBeNull();
    });

    unmount();
  });

  it("vim: F2 on the focused active perspective tab mounts the inline rename editor", async () => {
    mockKeymapMode = "vim";
    const { container, unmount } = await renderInAppShell();
    await flushSetup();

    await focusTab(container, "perspective_tab:p1");

    await pressKeyInAct("{F2}");
    await flushSetup();

    await waitFor(() => {
      const renameEditor = container.querySelector(
        "[data-segment='perspective_tab:p1'] .cm-editor",
      );
      expect(renameEditor).not.toBeNull();
    });

    unmount();
  });

  it("emacs: F2 on the focused active perspective tab mounts the inline rename editor", async () => {
    mockKeymapMode = "emacs";
    const { container, unmount } = await renderInAppShell();
    await flushSetup();

    await focusTab(container, "perspective_tab:p1");

    await pressKeyInAct("{F2}");
    await flushSetup();

    await waitFor(() => {
      const renameEditor = container.querySelector(
        "[data-segment='perspective_tab:p1'] .cm-editor",
      );
      expect(renameEditor).not.toBeNull();
    });

    unmount();
  });

  // -------------------------------------------------------------------------
  // Test #6 — F2 on a focused INACTIVE tab activates AND starts rename
  // -------------------------------------------------------------------------

  it("F2 on a focused inactive perspective tab activates that tab AND mounts the rename editor", async () => {
    const { container, unmount } = await renderInAppShell();
    await flushSetup();

    // Active perspective is `p1`; spatially focus the inactive `p2` tab.
    await focusTab(container, "perspective_tab:p2");

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await pressKeyInAct("{F2}");
    await flushSetup();

    // The per-tab `app.entity.startRename` execute:
    //   1. Dispatches `perspective.switch` for the focused (inactive) tab.
    //   2. Calls `triggerStartRename(p2.id)` so the rename editor mounts
    //      on this tab even before the activate's UI-state event has
    //      propagated.
    const setDispatch = findDispatch("perspective.switch");
    expect(setDispatch).toBeTruthy();
    expect(setDispatch!.args).toEqual(
      expect.objectContaining({ perspective_id: "p2" }),
    );

    // The inline editor mounts on the focused tab (p2), not on the
    // currently-active tab (p1).
    await waitFor(() => {
      const renameEditor = container.querySelector(
        "[data-segment='perspective_tab:p2'] .cm-editor",
      );
      expect(renameEditor).not.toBeNull();
    });
    expect(
      container.querySelectorAll(
        "[data-segment='perspective_tab:p1'] .cm-editor",
      ),
    ).toHaveLength(0);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Test #7 — Commit path: F2, typing + Enter dispatches perspective.rename
  // -------------------------------------------------------------------------

  it("commit path: typing in the rename editor and pressing Enter dispatches perspective.rename", async () => {
    const { container, unmount } = await renderInAppShell();
    await flushSetup();

    await focusTab(container, "perspective_tab:p1");

    // F2 mounts the rename editor.
    await pressKeyInAct("{F2}");
    await flushSetup();

    const renameEditor = await waitFor(() => {
      const ed = container.querySelector(
        "[data-segment='perspective_tab:p1'] .cm-editor",
      );
      expect(ed).not.toBeNull();
      return ed as HTMLElement;
    });

    // Type new text into the CM6 view and press Enter — the inner submit
    // keymap (built by `buildSubmitCancelExtensions` with
    // `alwaysSubmitOnEnter: true`) commits and the wrapper dispatches
    // `perspective.rename`. The CM6 editor owns focus, so the outer
    // global Enter binding does NOT fire — that is the regression guard
    // this case pins.
    const { EditorView } = await import("@codemirror/view");
    const view = EditorView.findFromDOM(renameEditor);
    expect(view).toBeTruthy();
    await act(async () => {
      view!.dispatch({
        changes: {
          from: 0,
          to: view!.state.doc.length,
          insert: "Sprint Edited",
        },
      });
      await new Promise((r) => setTimeout(r, 20));
    });

    // Reset the dispatch_command spy so we measure only the commit's IPC.
    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    const cmContent = renameEditor.querySelector(".cm-content") as HTMLElement;
    await act(async () => {
      cmContent.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "Enter",
          bubbles: true,
          cancelable: true,
        }),
      );
      await new Promise((r) => setTimeout(r, 50));
    });

    const renameDispatch = findDispatch("perspective.rename");
    expect(renameDispatch).toBeTruthy();
    expect(renameDispatch!.args).toEqual(
      expect.objectContaining({ id: "p1", new_name: "Sprint Edited" }),
    );

    unmount();
  });

  // -------------------------------------------------------------------------
  // Test #8 — Escape preserves existing per-keymap policy
  // -------------------------------------------------------------------------

  it("cua: Escape inside the rename editor cancels (no perspective.rename dispatch)", async () => {
    const { container, unmount } = await renderInAppShell();
    await flushSetup();

    await focusTab(container, "perspective_tab:p1");

    await pressKeyInAct("{F2}");
    await flushSetup();

    const renameEditor = await waitFor(() => {
      const ed = container.querySelector(
        "[data-segment='perspective_tab:p1'] .cm-editor",
      );
      expect(ed).not.toBeNull();
      return ed as HTMLElement;
    });

    // Type some new text but cancel before commit.
    const { EditorView } = await import("@codemirror/view");
    const view = EditorView.findFromDOM(renameEditor);
    await act(async () => {
      view!.dispatch({
        changes: { from: 0, to: view!.state.doc.length, insert: "Discard" },
      });
      await new Promise((r) => setTimeout(r, 20));
    });

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    const cmContent = renameEditor.querySelector(".cm-content") as HTMLElement;
    await act(async () => {
      cmContent.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "Escape",
          bubbles: true,
          cancelable: true,
        }),
      );
      await new Promise((r) => setTimeout(r, 50));
    });

    // No `perspective.rename` dispatch in cua mode.
    expect(dispatchedCommand("perspective.rename")).toBe(false);

    unmount();
  });

  it("vim: Escape inside the rename editor commits (matches existing useInlineRenamePolicy)", async () => {
    mockKeymapMode = "vim";
    const { container, unmount } = await renderInAppShell();
    await flushSetup();

    await focusTab(container, "perspective_tab:p1");

    await pressKeyInAct("{F2}");
    await flushSetup();

    const renameEditor = await waitFor(() => {
      const ed = container.querySelector(
        "[data-segment='perspective_tab:p1'] .cm-editor",
      );
      expect(ed).not.toBeNull();
      return ed as HTMLElement;
    });

    // Vim normal mode treats Escape as "commit what I have."
    const { EditorView } = await import("@codemirror/view");
    const view = EditorView.findFromDOM(renameEditor);
    await act(async () => {
      view!.dispatch({
        changes: {
          from: 0,
          to: view!.state.doc.length,
          insert: "Sprint Vim",
        },
      });
      await new Promise((r) => setTimeout(r, 20));
    });

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    const cmContent = renameEditor.querySelector(".cm-content") as HTMLElement;
    await act(async () => {
      cmContent.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "Escape",
          bubbles: true,
          cancelable: true,
        }),
      );
      await new Promise((r) => setTimeout(r, 50));
    });

    const renameDispatch = findDispatch("perspective.rename");
    expect(renameDispatch).toBeTruthy();
    expect(renameDispatch!.args).toEqual(
      expect.objectContaining({ id: "p1", new_name: "Sprint Vim" }),
    );

    unmount();
  });

  it("emacs: Escape inside the rename editor cancels (no perspective.rename dispatch)", async () => {
    mockKeymapMode = "emacs";
    const { container, unmount } = await renderInAppShell();
    await flushSetup();

    await focusTab(container, "perspective_tab:p1");

    await pressKeyInAct("{F2}");
    await flushSetup();

    const renameEditor = await waitFor(() => {
      const ed = container.querySelector(
        "[data-segment='perspective_tab:p1'] .cm-editor",
      );
      expect(ed).not.toBeNull();
      return ed as HTMLElement;
    });

    const { EditorView } = await import("@codemirror/view");
    const view = EditorView.findFromDOM(renameEditor);
    await act(async () => {
      view!.dispatch({
        changes: { from: 0, to: view!.state.doc.length, insert: "Discard" },
      });
      await new Promise((r) => setTimeout(r, 20));
    });

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    const cmContent = renameEditor.querySelector(".cm-content") as HTMLElement;
    await act(async () => {
      cmContent.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "Escape",
          bubbles: true,
          cancelable: true,
        }),
      );
      await new Promise((r) => setTimeout(r, 50));
    });

    // Emacs follows cua semantics: Escape cancels.
    expect(dispatchedCommand("perspective.rename")).toBe(false);

    unmount();
  });
});
