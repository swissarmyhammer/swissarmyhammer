/**
 * End-to-end frontend verification for the perspective delete-undo flow.
 *
 * The Rust integration test
 * `perspective_delete_undo_restores_cache_and_emits_event` pins the backend
 * contract: after `app.undo` reverses a `perspective.delete`, the cache is
 * re-populated and a `PerspectiveChanged` event fires. The Tauri bridge in
 * `kanban-app/src/watcher.rs::process_perspective_event` translates that
 * event into an `entity-field-changed` Tauri event with `changes: []`.
 *
 * This test verifies the final hop — that the frontend picks up that event
 * and actually re-renders the deleted perspective's tab. Without this
 * coverage, a regression in the `perspective-context` event listener (e.g.
 * dropping refetch-on-empty-changes) would leave the Rust tests green but
 * break the user-visible flow.
 *
 * The test mounts `<PerspectiveTabBar>` inside the real `<PerspectiveProvider>`
 * and drives it against a mocked Tauri bridge (`dispatch_command` + `listen`).
 * Other ancillary contexts (schema, UIState, views, entity store, context
 * menu, board data) are stubbed at the module boundary because they are
 * orthogonal to the delete-undo flow and would otherwise require a full
 * App-scale mount.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import type { ReactNode } from "react";
import { TooltipProvider } from "@/components/ui/tooltip";

// Mock Tauri invoke before importing any module that uses it. Each call is
// recorded so we can assert the refetch count after the undo event.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((..._args: any[]): Promise<any> => Promise.resolve(null));
vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...args),
}));

// Capture listen() callbacks per event so the test can fire a synthetic
// post-undo event straight into the provider's listener.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
let listenCallbacks: Record<string, (event: { payload: any }) => void> = {};
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(
    (
      event: string,
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      cb: (e: { payload: any }) => void,
    ) => {
      listenCallbacks[event] = cb;
      return Promise.resolve(() => {});
    },
  ),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

// UIState — minimal shape; no active perspective so the provider falls back
// to the first one in the list. Kept as a stable object so hook identities
// do not churn on every render.
const mockUIState = {
  keymap_mode: "cua",
  scope_chain: [],
  open_boards: [],
  has_clipboard: false,
  clipboard_entity_type: null,
  windows: {} as Record<string, { active_perspective_id: string }>,
  recent_boards: [],
};

vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => mockUIState,
  useUIStateLoading: () => ({ state: mockUIState, loading: false }),
}));

// Views — one board view; the PerspectiveTabBar filters perspectives by
// `view === activeView.kind` so both seeded perspectives must be `board`.
vi.mock("@/lib/views-context", () => ({
  useViews: () => ({
    views: [{ id: "board-1", name: "Board", kind: "board", icon: "kanban" }],
    activeView: { id: "board-1", name: "Board", kind: "board", icon: "kanban" },
    setActiveViewId: vi.fn(),
    refresh: vi.fn(() => Promise.resolve()),
  }),
}));

// Context menu — returns a handler that records calls; unused here, but
// PerspectiveTabBar wires it unconditionally.
vi.mock("@/lib/context-menu", () => ({
  useContextMenu: () => vi.fn(),
}));

// EntityStore + Schema + BoardData — all consumed transitively by
// FilterEditor / GroupSelector. Stub them to empty so the components mount.
vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({ getEntities: () => [] }),
}));

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => ({ entity: { name: "task", fields: [] }, fields: [] }),
    getFieldDef: () => undefined,
    mentionableTypes: [],
    loading: false,
  }),
}));

vi.mock("@/components/window-container", () => ({
  useBoardData: () => ({ virtualTagMeta: [] }),
}));

import { PerspectiveProvider } from "@/lib/perspective-context";
import {
  CommandScopeProvider,
  ActiveBoardPathProvider,
} from "@/lib/command-scope";
import { PerspectiveTabBar } from "./perspective-tab-bar";
import type { PerspectiveDef } from "@/types/kanban";

/** Build a minimal board-kind PerspectiveDef for test use. */
function makePerspective(id: string, name: string): PerspectiveDef {
  return { id, name, view: "board" };
}

/**
 * Count the perspective tab buttons currently rendered. Tabs are the
 * elements that wrap the perspective name text — we locate them via the
 * visible name and walk up to the nearest button. This mirrors how a user
 * would see "two tabs" vs. "one tab" on screen.
 */
function countRenderedTabs(names: readonly string[]): number {
  return names.reduce((acc, name) => {
    const el = screen.queryByText(name);
    return el ? acc + 1 : acc;
  }, 0);
}

/**
 * Render `<PerspectiveTabBar>` under the real `<PerspectiveProvider>`,
 * wired to the mocked Tauri bridge. The `CommandScopeProvider` gives the
 * provider its window moniker (needed for `dispatch_command` args) and the
 * `ActiveBoardPathProvider` supplies the board path the provider passes
 * along on every dispatch. The `TooltipProvider` is a hard requirement for
 * the tab bar's hover tooltips.
 */
function renderWithProvider(): ReturnType<typeof render> {
  const wrapper = ({ children }: { children: ReactNode }) => (
    <CommandScopeProvider commands={[]} moniker="window:main">
      <ActiveBoardPathProvider value="/tmp/test/.kanban">
        <PerspectiveProvider>
          <TooltipProvider delayDuration={100}>{children}</TooltipProvider>
        </PerspectiveProvider>
      </ActiveBoardPathProvider>
    </CommandScopeProvider>
  );
  return render(<PerspectiveTabBar />, { wrapper });
}

describe("PerspectiveTabBar delete-undo flow", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listenCallbacks = {};
  });

  it("re-renders the deleted tab after a post-undo entity-field-changed event", async () => {
    // --- Step 1: seed two perspectives and render the tab bar. ---
    //
    // Both perspectives are `view: "board"` so they match the active view
    // kind and both render as tabs. The provider fetches on mount via
    // `perspective.list`.
    const pid = "01AAAAAAAAAAAAAAAAAAAAAAAA";
    const survivorId = "01BBBBBBBBBBBBBBBBBBBBBBBB";
    const initial = [
      makePerspective(pid, "Doomed"),
      makePerspective(survivorId, "Survivor"),
    ];
    mockInvoke.mockResolvedValue({
      result: { perspectives: initial, count: 2 },
      undoable: false,
    });

    renderWithProvider();

    // Flush the provider's initial fetch so both tabs land in the DOM.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });

    expect(countRenderedTabs(["Doomed", "Survivor"])).toBe(2);

    // --- Step 2: simulate the delete. ---
    //
    // The perspective-context listener refetches on `entity-removed` for the
    // perspective entity type. Prime the next `perspective.list` response to
    // reflect the post-delete state (only Survivor), then fire the event.
    mockInvoke.mockResolvedValue({
      result: { perspectives: [makePerspective(survivorId, "Survivor")], count: 1 },
      undoable: false,
    });

    await act(async () => {
      listenCallbacks["entity-removed"]?.({
        payload: { entity_type: "perspective", id: pid },
      });
      await new Promise((r) => setTimeout(r, 0));
    });

    // Tab count drops to one — the "Doomed" tab is gone, "Survivor" stays.
    expect(countRenderedTabs(["Doomed", "Survivor"])).toBe(1);
    expect(screen.queryByText("Doomed")).toBeNull();
    expect(screen.queryByText("Survivor")).not.toBeNull();

    // --- Step 3: simulate the post-undo bridge event. ---
    //
    // After `app.undo` reverses the delete, the backend rewrites the YAML,
    // `PerspectiveContext::reload_from_disk` re-inserts the cache entry, and
    // the bridge translates the resulting `PerspectiveChanged` event into
    // `entity-field-changed` with an empty `changes` array (the wire shape
    // for `changed_fields: []`). Prime the next list response to reflect the
    // restored state.
    const afterUndo = [
      makePerspective(pid, "Doomed"),
      makePerspective(survivorId, "Survivor"),
    ];
    mockInvoke.mockResolvedValue({
      result: { perspectives: afterUndo, count: 2 },
      undoable: false,
    });

    // Snapshot the current dispatch call count so we can assert that the
    // event triggers exactly one additional `perspective.list`.
    const listCallsBefore = mockInvoke.mock.calls.filter(
      (call) =>
        call[0] === "dispatch_command" &&
        (call[1] as { cmd?: string })?.cmd === "perspective.list",
    ).length;

    await act(async () => {
      listenCallbacks["entity-field-changed"]?.({
        payload: {
          entity_type: "perspective",
          id: pid,
          // Empty changes — exact shape the bridge emits after
          // reload_from_disk with changed_fields=[].
          changes: [],
        },
      });
      await new Promise((r) => setTimeout(r, 0));
    });

    // --- Assertions ---
    //
    // 1. `perspective.list` was invoked exactly once more than before the
    //    event. The provider's listener doesn't patch the list in place —
    //    it always refetches.
    const listCallsAfter = mockInvoke.mock.calls.filter(
      (call) =>
        call[0] === "dispatch_command" &&
        (call[1] as { cmd?: string })?.cmd === "perspective.list",
    ).length;
    expect(listCallsAfter).toBe(listCallsBefore + 1);

    // 2. The restored tab is back on screen. Both perspectives render again.
    expect(countRenderedTabs(["Doomed", "Survivor"])).toBe(2);
    expect(screen.queryByText("Doomed")).not.toBeNull();
  });
});
