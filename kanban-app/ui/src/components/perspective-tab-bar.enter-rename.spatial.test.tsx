/**
 * Browser-mode test for the spatial-focus → Enter → inline-rename keyboard
 * path on `<PerspectiveTabBar>`.
 *
 * Source of truth for acceptance of card `01KQ7GE3KY91X2YR6BX5AY40VK`
 * ("Fix: Enter on focused perspective tab does not start inline rename").
 *
 * Background. Before this card the rename trigger
 * `triggerStartRename()` had only two callers: the command palette's
 * `ui.entity.startRename` row and the `onDoubleClick` handler on each tab.
 * Pressing Enter while a perspective tab was the spatial focus fell through
 * to the global `nav.drillIn: Enter` binding, which is a no-op for a leaf
 * scope. The fix scopes a fresh `ui.entity.startRename` `CommandDef` (with
 * `keys: { cua: "Enter", vim: "Enter", emacs: "Enter" }`) onto the active
 * tab's `<CommandScopeProvider>` so `extractScopeBindings` claims Enter
 * away from the global drill-in binding while the active perspective tab is
 * focused — and ONLY there.
 *
 * Test cases (per the card's "Browser Tests (mandatory)" section):
 *
 * 1. **Enter triggers rename** — focusing the active perspective tab and
 *    pressing Enter mounts the inline `<InlineRenameEditor>` (a CM6
 *    `.cm-editor`) inside the active tab's button.
 * 2. **Enter on inactive tab is a no-op for rename** — focusing a perspective
 *    tab whose `id !== activePerspective.id` and pressing Enter does NOT
 *    mount any rename editor on any perspective tab. (The active-tab-only
 *    binding registration is what keeps inactive tabs from claiming Enter.)
 * 3. **Enter outside perspective scope still drills** — focusing a non-
 *    perspective leaf and pressing Enter dispatches `spatial_drill_in` and
 *    no `.cm-editor` mounts inside the perspective bar. Proves the binding
 *    is scope-local.
 * 4. **Vim Enter** — same as case 1 with `keymap_mode: "vim"`.
 * 5. **Emacs Enter** — same as case 1 with `keymap_mode: "emacs"`.
 * 6. **Commit path still works** — after case 1 mounts the rename editor,
 *    typing into the editor and pressing Enter dispatches
 *    `perspective.rename` with `{ id, new_name }`. Regression guard for the
 *    inner CM6 keymap winning over the outer scope-bound Enter.
 * 7. **Escape preserves existing policy** — after case 1 mounts the editor,
 *    Escape in cua/emacs cancels (no rename dispatch), while Escape in vim
 *    normal mode commits per the existing `useInlineRenamePolicy` contract.
 *
 * Mock pattern matches `column-view.spatial.test.tsx` /
 * `perspective-bar.spatial.test.tsx`: `vi.hoisted` builds the
 * `mockInvoke` / `mockListen` / `listeners` triple; `fireFocusChanged`
 * drives the React tree as if the Rust kernel emitted a `focus-changed`
 * event for the captured spatial key.
 *
 * Runs under `kanban-app/ui/vite.config.ts`'s browser project (real
 * Chromium via Playwright).
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act, waitFor } from "@testing-library/react";
import { userEvent } from "vitest/browser";
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
  type WindowLabel
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
 *
 * The payload's `prev_fq` / `next_fq` mirror the kernel's emit shape
 * after `spatial_focus` / `spatial_navigate`. Wrapping the dispatch in
 * `act()` flushes React state updates so callers can assert against
 * post-update DOM in the next tick.
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
 * Render the bar inside the full `<AppShell>` provider stack.
 *
 * AppShell wires the global keydown listener that drives the
 * `nav.drillIn` / `ui.entity.startRename` commands, so tests for
 * keystroke → rename / drill-in need that wiring to fire on
 * `userEvent.keyboard()`.
 */
function renderInAppShell(extraChildren?: ReactElement) {
  return render(
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

/** Capture every `spatial_drill_in` call's args. */
function spatialDrillInCalls(): Array<{ key: FullyQualifiedMoniker }> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_drill_in")
    .map((c) => c[1] as { key: FullyQualifiedMoniker });
}

/** True when any registered scope has the given moniker. */
function findScopeKey(moniker: string): FullyQualifiedMoniker | undefined {
  const scope = registerScopeArgs().find((a) => a.segment === moniker);
  return scope?.key as FullyQualifiedMoniker | undefined;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("PerspectiveTabBar — Enter on focused tab triggers inline rename", () => {
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
  // Test #1 — Enter on the focused active tab triggers inline rename
  // -------------------------------------------------------------------------

  it("Enter on the focused active perspective tab mounts the inline rename editor", async () => {
    const { container, unmount } = renderInAppShell();
    await flushSetup();

    // The rename editor is NOT mounted before Enter. The active perspective
    // is `p1`, and the formula bar contributes one CM6 editor on the right
    // side — so we expect at most one `.cm-editor` and assert no rename
    // editor is inside any tab button.
    expect(
      container.querySelectorAll(
        "[data-moniker^='perspective_tab:'] .cm-editor",
      ),
    ).toHaveLength(0);

    const p1Key = findScopeKey("perspective_tab:p1");
    expect(p1Key).toBeTruthy();

    await fireFocusChanged({
      next_fq: p1Key!,
      next_segment: asSegment("perspective_tab:p1"),
    });

    await waitFor(() => {
      const focusedTab = container.querySelector(
        "[data-moniker='perspective_tab:p1']",
      );
      expect(focusedTab?.getAttribute("data-focused")).toBe("true");
    });

    // Press Enter at the document level — the global keymap handler picks
    // it up and dispatches `ui.entity.startRename`, which the AppShell's
    // execute closure forwards to `triggerStartRename()`.
    await userEvent.keyboard("{Enter}");
    await flushSetup();

    // The active tab now hosts the inline CM6 rename editor.
    await waitFor(() => {
      const renameEditor = container.querySelector(
        "[data-moniker='perspective_tab:p1'] .cm-editor",
      );
      expect(renameEditor).not.toBeNull();
    });

    unmount();
  });

  // -------------------------------------------------------------------------
  // Test #2 — Enter on a focused INACTIVE tab does NOT trigger rename
  // -------------------------------------------------------------------------

  it("Enter on a focused inactive perspective tab does NOT mount any rename editor", async () => {
    const { container, unmount } = renderInAppShell();
    await flushSetup();

    // Active perspective is `p1`; spatially focus the inactive `p2` tab.
    const p2Key = findScopeKey("perspective_tab:p2");
    expect(p2Key).toBeTruthy();

    await fireFocusChanged({
      next_fq: p2Key!,
      next_segment: asSegment("perspective_tab:p2"),
    });

    await waitFor(() => {
      const focusedTab = container.querySelector(
        "[data-moniker='perspective_tab:p2']",
      );
      expect(focusedTab?.getAttribute("data-focused")).toBe("true");
    });

    await userEvent.keyboard("{Enter}");
    await flushSetup();

    // No rename editor mounts on ANY perspective tab — the active-tab-only
    // binding registration in `ScopedPerspectiveTab` keeps the inactive
    // tab's scope free of `ui.entity.startRename`. Enter falls through to
    // the global `nav.drillIn`, which is a no-op for a leaf scope.
    expect(
      container.querySelectorAll(
        "[data-moniker^='perspective_tab:'] .cm-editor",
      ),
    ).toHaveLength(0);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Test #3 — Enter outside the perspective scope still drills in
  // -------------------------------------------------------------------------

  it("Enter on a non-perspective focused leaf still dispatches spatial_drill_in", async () => {
    // Mount a non-perspective FocusScope (`task:01ABC`) alongside the
    // perspective bar so the test can drive focus to a leaf that has
    // nothing to do with the perspective scope chain. Enter while that
    // leaf is focused MUST hit the global `nav.drillIn` binding — proving
    // the new `ui.entity.startRename: Enter` binding is scope-local and
    // does not leak to other focus contexts.
    const { container, unmount } = renderInAppShell(
      <FocusScope moniker={asSegment("task:01ABC")} commands={[]}>
        <div data-testid="non-perspective-leaf">leaf</div>
      </FocusScope>,
    );
    await flushSetup();

    const taskKey = registerScopeArgs().find(
      (a) => a.segment === "task:01ABC",
    )?.key as FullyQualifiedMoniker | undefined;
    expect(taskKey).toBeTruthy();

    await fireFocusChanged({
      next_fq: taskKey!,
      next_segment: asSegment("task:01ABC"),
    });

    await waitFor(() => {
      const focused = container.querySelector(
        "[data-moniker='task:01ABC']",
      );
      expect(focused?.getAttribute("data-focused")).toBe("true");
    });

    // Reset invoke calls so the assertion measures only Enter's IPC.
    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await userEvent.keyboard("{Enter}");
    await flushSetup();

    // Enter dispatched to the kernel as a drill-in for the focused leaf.
    expect(spatialDrillInCalls()).toHaveLength(1);
    expect(spatialDrillInCalls()[0].key).toBe(taskKey);

    // No rename editor mounted in any perspective tab.
    expect(
      container.querySelectorAll(
        "[data-moniker^='perspective_tab:'] .cm-editor",
      ),
    ).toHaveLength(0);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Test #4 — Vim Enter triggers rename on the focused active tab
  // -------------------------------------------------------------------------

  it("vim: Enter on the focused active perspective tab mounts the inline rename editor", async () => {
    mockKeymapMode = "vim";
    const { container, unmount } = renderInAppShell();
    await flushSetup();

    const p1Key = findScopeKey("perspective_tab:p1");
    expect(p1Key).toBeTruthy();

    await fireFocusChanged({
      next_fq: p1Key!,
      next_segment: asSegment("perspective_tab:p1"),
    });
    await waitFor(() => {
      const focusedTab = container.querySelector(
        "[data-moniker='perspective_tab:p1']",
      );
      expect(focusedTab?.getAttribute("data-focused")).toBe("true");
    });

    await userEvent.keyboard("{Enter}");
    await flushSetup();

    await waitFor(() => {
      const renameEditor = container.querySelector(
        "[data-moniker='perspective_tab:p1'] .cm-editor",
      );
      expect(renameEditor).not.toBeNull();
    });

    unmount();
  });

  // -------------------------------------------------------------------------
  // Test #5 — Emacs Enter triggers rename on the focused active tab
  // -------------------------------------------------------------------------

  it("emacs: Enter on the focused active perspective tab mounts the inline rename editor", async () => {
    mockKeymapMode = "emacs";
    const { container, unmount } = renderInAppShell();
    await flushSetup();

    const p1Key = findScopeKey("perspective_tab:p1");
    expect(p1Key).toBeTruthy();

    await fireFocusChanged({
      next_fq: p1Key!,
      next_segment: asSegment("perspective_tab:p1"),
    });
    await waitFor(() => {
      const focusedTab = container.querySelector(
        "[data-moniker='perspective_tab:p1']",
      );
      expect(focusedTab?.getAttribute("data-focused")).toBe("true");
    });

    await userEvent.keyboard("{Enter}");
    await flushSetup();

    await waitFor(() => {
      const renameEditor = container.querySelector(
        "[data-moniker='perspective_tab:p1'] .cm-editor",
      );
      expect(renameEditor).not.toBeNull();
    });

    unmount();
  });

  // -------------------------------------------------------------------------
  // Test #6 — Commit path: typing + Enter dispatches perspective.rename
  // -------------------------------------------------------------------------

  it("commit path: typing in the rename editor and pressing Enter dispatches perspective.rename", async () => {
    const { container, unmount } = renderInAppShell();
    await flushSetup();

    const p1Key = findScopeKey("perspective_tab:p1");
    expect(p1Key).toBeTruthy();

    await fireFocusChanged({
      next_fq: p1Key!,
      next_segment: asSegment("perspective_tab:p1"),
    });
    await waitFor(() => {
      const focusedTab = container.querySelector(
        "[data-moniker='perspective_tab:p1']",
      );
      expect(focusedTab?.getAttribute("data-focused")).toBe("true");
    });

    // Outer Enter mounts the rename editor.
    await userEvent.keyboard("{Enter}");
    await flushSetup();

    const renameEditor = await waitFor(() => {
      const ed = container.querySelector(
        "[data-moniker='perspective_tab:p1'] .cm-editor",
      );
      expect(ed).not.toBeNull();
      return ed as HTMLElement;
    });

    // Type new text into the CM6 view and press Enter — the inner submit
    // keymap (built by `buildSubmitCancelExtensions` with
    // `alwaysSubmitOnEnter: true`) commits and the wrapper dispatches
    // `perspective.rename`. The CM6 editor owns focus, so the outer
    // scope-bound Enter binding does NOT fire — that is the regression
    // guard this case pins.
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
  // Test #7 — Escape preserves existing per-keymap policy
  // -------------------------------------------------------------------------

  it("cua: Escape inside the rename editor cancels (no perspective.rename dispatch)", async () => {
    const { container, unmount } = renderInAppShell();
    await flushSetup();

    const p1Key = findScopeKey("perspective_tab:p1");
    expect(p1Key).toBeTruthy();

    await fireFocusChanged({
      next_fq: p1Key!,
      next_segment: asSegment("perspective_tab:p1"),
    });
    await waitFor(() => {
      const focusedTab = container.querySelector(
        "[data-moniker='perspective_tab:p1']",
      );
      expect(focusedTab?.getAttribute("data-focused")).toBe("true");
    });

    await userEvent.keyboard("{Enter}");
    await flushSetup();

    const renameEditor = await waitFor(() => {
      const ed = container.querySelector(
        "[data-moniker='perspective_tab:p1'] .cm-editor",
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
    const { container, unmount } = renderInAppShell();
    await flushSetup();

    const p1Key = findScopeKey("perspective_tab:p1");
    expect(p1Key).toBeTruthy();

    await fireFocusChanged({
      next_fq: p1Key!,
      next_segment: asSegment("perspective_tab:p1"),
    });
    await waitFor(() => {
      const focusedTab = container.querySelector(
        "[data-moniker='perspective_tab:p1']",
      );
      expect(focusedTab?.getAttribute("data-focused")).toBe("true");
    });

    await userEvent.keyboard("{Enter}");
    await flushSetup();

    const renameEditor = await waitFor(() => {
      const ed = container.querySelector(
        "[data-moniker='perspective_tab:p1'] .cm-editor",
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
    const { container, unmount } = renderInAppShell();
    await flushSetup();

    const p1Key = findScopeKey("perspective_tab:p1");
    expect(p1Key).toBeTruthy();

    await fireFocusChanged({
      next_fq: p1Key!,
      next_segment: asSegment("perspective_tab:p1"),
    });
    await waitFor(() => {
      const focusedTab = container.querySelector(
        "[data-moniker='perspective_tab:p1']",
      );
      expect(focusedTab?.getAttribute("data-focused")).toBe("true");
    });

    await userEvent.keyboard("{Enter}");
    await flushSetup();

    const renameEditor = await waitFor(() => {
      const ed = container.querySelector(
        "[data-moniker='perspective_tab:p1'] .cm-editor",
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

  // -------------------------------------------------------------------------
  // Test #8 — regression guard for card 01KQ9X3A9NMRYK50GWP4S4ZMJ4
  //
  // After the Enter-drill-in card removed `board.inspect`'s vim Enter
  // binding and added a per-field-zone `field.edit: Enter` binding, the
  // perspective tab's scope-pinned `ui.entity.startRename: Enter` must
  // continue to fire on the active tab regardless of which keymap is
  // active. The pre-existing cua / vim / emacs tests above already
  // assert this; this additional case pins the regression explicitly so
  // a future edit to either of the two precision-targeted scope bindings
  // (field zone or board) cannot silently swallow this one.
  // -------------------------------------------------------------------------

  it("regression: scope-pinned ui.entity.startRename: vim Enter still fires after Enter-drill-in card", async () => {
    mockKeymapMode = "vim";
    const { container, unmount } = renderInAppShell();
    await flushSetup();

    const p1Key = findScopeKey("perspective_tab:p1");
    expect(p1Key).toBeTruthy();

    await fireFocusChanged({
      next_fq: p1Key!,
      next_segment: asSegment("perspective_tab:p1"),
    });
    await waitFor(() => {
      const focusedTab = container.querySelector(
        "[data-moniker='perspective_tab:p1']",
      );
      expect(focusedTab?.getAttribute("data-focused")).toBe("true");
    });

    await userEvent.keyboard("{Enter}");
    await flushSetup();

    // The active perspective tab's scope still carries the
    // `ui.entity.startRename: Enter` binding; pressing Enter mounts
    // the inline rename editor inside the tab. Regression failure
    // would manifest either as no editor mounting (drill-in or
    // field.edit shadowed it) or as a `ui.inspect` dispatch (a
    // resurrected `board.inspect` shadow).
    await waitFor(() => {
      const renameEditor = container.querySelector(
        "[data-moniker='perspective_tab:p1'] .cm-editor",
      );
      expect(renameEditor).not.toBeNull();
    });

    // No `ui.inspect` dispatch must appear — Enter belongs to rename
    // here, not inspect.
    expect(dispatchedCommand("ui.inspect")).toBe(false);

    unmount();
  });
});
