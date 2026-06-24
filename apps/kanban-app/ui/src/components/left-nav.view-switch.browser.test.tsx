/**
 * Regression tests for the full view-switch loop (card
 * 01KTCRX1AP2WHKM4BPHWG7XYJT — "Cannot switch between views").
 *
 * Unlike `left-nav.browser.test.tsx` (which mocks `views-context`), these
 * tests mount the REAL `UIStateProvider` + `ViewsProvider` + `LeftNav`
 * stack so they pin both halves of the production loop:
 *
 * 1. **Producer guarantee** — clicking a view button dispatches `view.set`
 *    with a scope chain that carries the ambient `window:<label>` moniker.
 *    The backend's `set active_view` op resolves its target window from
 *    that moniker and (post-hardening, card 01KTECWA8D05FVKJ80MA3H0FFY)
 *    REJECTS the call when it is missing. The click handler swallows
 *    dispatch errors (`.catch(console.error)`), so a producer that loses
 *    the window moniker silently breaks view switching — exactly this
 *    card's symptom. The chain must also carry the `view:{id}` moniker:
 *    the backend rewrites `view:*` entries in the recorded focus chain so
 *    the palette keeps offering view-scoped commands.
 *
 * 2. **Consumer loop** — the backend answers a successful `view.set` with
 *    a `notifications/ui_state/changed` event (`kind: "active_view"`, full per-window
 *    snapshot). The frontend must apply it (NOT treat `active_view` as a
 *    frontend-authoritative kind), `ViewsProvider` must read THIS window's
 *    `active_view_id` slice, and the left-nav highlight must move.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import type { RenderResult } from "@testing-library/react";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { ViewDef } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Mocks — Tauri APIs must be mocked before importing components that pull
// them in transitively. `listeners` records every `listen(event, callback)`
// registration so tests can fire backend events by name.
// ---------------------------------------------------------------------------

const { mockInvoke, listeners } = vi.hoisted(() => {
  type Listener = (event: { payload: unknown }) => void;
  const listeners = new Map<string, Listener[]>();
  const mockInvoke = vi.fn(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any -- Tauri invoke arguments are command-specific and untyped at this mock boundary
    (_command: string, _arguments?: any): Promise<unknown> =>
      Promise.resolve(null),
  );
  return { mockInvoke, listeners };
});

vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any -- forwards the real `invoke` variadic signature, whose argument types vary by command
  invoke: (...args: any[]) => mockInvoke(...(args as [string, unknown])),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (event: string, callback: (event: { payload: unknown }) => void) => {
    const list = listeners.get(event) ?? [];
    list.push(callback);
    listeners.set(event, list);
    return Promise.resolve(() => {
      const currentCallbacks = listeners.get(event) ?? [];
      listeners.set(
        event,
        currentCallbacks.filter((registered) => registered !== callback),
      );
    });
  },
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

// Import after mocks so the mocked module bindings are the ones used.
import { LeftNav } from "./left-nav";
import { UI_STATE_CHANGED_EVENT } from "@/lib/mcp-notifications";
import { UIStateProvider } from "@/lib/ui-state-context";
import type { UIStateSnapshot } from "@/lib/ui-state-context";
import { ViewsProvider } from "@/lib/views-context";
import { CommandScopeProvider } from "@/lib/command-scope";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "./focus-layer";
import { asSegment } from "@/types/spatial";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const V1: ViewDef = { id: "v1", name: "View 1", kind: "board", icon: "kanban" };
const V2: ViewDef = { id: "v2", name: "View 2", kind: "grid", icon: "table" };

/** Full UIState snapshot with one `main` window slot. */
function uiStateSnapshot(activeViewId: string): UIStateSnapshot {
  return {
    keymap_mode: "cua",
    scope_chain: [],
    open_boards: [],
    has_clipboard: false,
    clipboard_entity_type: null,
    windows: {
      main: {
        board_path: "",
        inspector_stack: [],
        active_view_id: activeViewId,
        active_perspective_id: "",
        palette_open: false,
        palette_mode: "command",
        app_mode: "normal",
      },
    },
    recent_boards: [],
  };
}

/** Fire a backend event into every registered `listen` subscriber. */
function emitTauriEvent(event: string, payload: unknown): void {
  act(() => {
    listeners.get(event)?.forEach((callback) => callback({ payload }));
  });
}

/**
 * Render LeftNav inside the production-shaped provider nest: the spatial
 * stack, a `window:main` command scope (what `WindowContainer` provides in
 * `App.tsx`), and the REAL UIState + Views providers.
 */
function renderLeftNavLoop(): RenderResult {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <CommandScopeProvider moniker="window:main">
          <UIStateProvider>
            <ViewsProvider>
              <TooltipProvider delayDuration={100}>
                <LeftNav />
              </TooltipProvider>
            </ViewsProvider>
          </UIStateProvider>
        </CommandScopeProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

/** The active-highlight class `ViewButton` applies to the active view. */
const ACTIVE_CLASS = "bg-primary";

describe("LeftNav — view-switch loop (view.set)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listeners.clear();
    mockInvoke.mockImplementation(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any -- Tauri invoke arguments are command-specific and untyped at this mock boundary
      (command: string, _arguments?: any): Promise<unknown> => {
        if (command === "get_ui_state") {
          return Promise.resolve(uiStateSnapshot("v1"));
        }
        if (command === "list_views") {
          return Promise.resolve([V1, V2]);
        }
        return Promise.resolve(null);
      },
    );
  });

  /**
   * Producer guarantee: the `view.set` dispatch must carry the ambient
   * `window:<label>` moniker (the backend resolves the target window from
   * it and rejects loudly when absent) plus the clicked view's `view:{id}`
   * moniker (the backend rewrites `view:*` chain entries to it).
   */
  it("clicking a view button dispatches view.set with a window-rooted scope chain", async () => {
    renderLeftNavLoop();

    const viewTwo = await screen.findByRole("button", { name: "View 2" });
    fireEvent.click(viewTwo);

    await act(async () => {});

    const viewSetCalls = mockInvoke.mock.calls.filter(
      ([cmd, args]) =>
        cmd === "dispatch_command" &&
        (args as { cmd?: string })?.cmd === "view.set",
    );
    expect(viewSetCalls).toHaveLength(1);
    const payload = viewSetCalls[0][1] as {
      cmd: string;
      args?: Record<string, unknown>;
      scopeChain?: string[];
    };
    expect(payload.args).toEqual({ view_id: "v2" });
    // The hardened backend (`set active_view`) errors without a `window:`
    // moniker, and the click handler swallows that error — so losing the
    // moniker here silently breaks view switching.
    expect(payload.scopeChain).toEqual(expect.arrayContaining(["window:main"]));
    // The clicked view's moniker rides along so the backend can rewrite
    // the recorded focus chain's `view:*` entries.
    expect(payload.scopeChain).toEqual(expect.arrayContaining(["view:v2"]));
  });

  /**
   * Context-menu scoping (card 01KV5K29FFQJTBER6HYA4J2DW6): right-clicking
   * view X surfaces exactly ITS OWN "Switch to View «X»" entry and nothing
   * for a sibling view Y. The backend's `commands_for_scope` flips only the
   * in-scope view's `view.set` row to `context_menu: true`; the frontend
   * filters the `list command` response to `context_menu === true` and hands
   * the survivors to the native `show context menu`. This drives the REAL
   * `ViewButton`/`useContextMenu` path: the right-click fires `list command`
   * with the clicked view's `view:{id}` in the scope chain, and the chosen
   * item dispatches `view.set` with that view's id.
   */
  it("right-clicking a view button shows only its own Switch to View entry", async () => {
    // The backend response for a right-click whose scope chain carries
    // `view:v2`: v2's row is context_menu:true (its own scoped entry), v1's
    // palette row stays context_menu:false. Both carry pre-filled view_id
    // args, exactly as `emit_view_switch` emits them.
    const listCommandResponse = {
      ok: true,
      commands: [
        {
          id: "view.set",
          name: "Switch to View View 1",
          context_menu: false,
          args: { view_id: "v1" },
        },
        {
          id: "view.set",
          name: "Switch to View View 2",
          context_menu: true,
          args: { view_id: "v2" },
        },
      ],
    };
    mockInvoke.mockImplementation(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any -- Tauri invoke arguments are command-specific and untyped at this mock boundary
      (cmd: string, args?: any): Promise<unknown> => {
        if (cmd === "get_ui_state") return Promise.resolve(uiStateSnapshot("v1"));
        if (cmd === "list_views") return Promise.resolve([V1, V2]);
        if (
          cmd === "command_tool_call" &&
          args?.module === "commands" &&
          args?.op === "list command"
        ) {
          return Promise.resolve(listCommandResponse);
        }
        return Promise.resolve(null);
      },
    );

    renderLeftNavLoop();

    const viewTwo = await screen.findByRole("button", { name: "View 2" });
    fireEvent.contextMenu(viewTwo);
    await act(async () => {});

    // The `list command` fetch must carry the clicked view's moniker so the
    // backend resolves v2's own context-menu row.
    const listCalls = mockInvoke.mock.calls.filter(
      ([cmd, args]) =>
        cmd === "command_tool_call" &&
        (args as { op?: string })?.op === "list command",
    );
    expect(listCalls.length).toBeGreaterThan(0);
    const listCtx = (
      listCalls[0][1] as { params?: { ctx?: { scope_chain?: string[] } } }
    ).params?.ctx;
    expect(listCtx?.scope_chain).toEqual(expect.arrayContaining(["view:v2"]));

    // The native `show context menu` must receive exactly v2's switch entry —
    // v1's palette row (context_menu:false) is filtered out by the frontend.
    const showMenuCall = mockInvoke.mock.calls.find(
      ([cmd, args]) =>
        cmd === "command_tool_call" &&
        (args as { module?: string; op?: string })?.module === "window" &&
        (args as { op?: string })?.op === "show context menu",
    );
    expect(showMenuCall).toBeDefined();
    const items = (
      showMenuCall![1] as {
        params: { items: { name: string; cmd: string; separator: boolean }[] };
      }
    ).params.items.filter((i) => !i.separator);
    expect(items.map((i) => i.name)).toEqual(["Switch to View View 2"]);
    expect(items.map((i) => i.name)).not.toContain("Switch to View View 1");
    expect(items[0].cmd).toBe("view.set");
  });

  /**
   * Consumer loop: a backend `notifications/ui_state/changed` event with
   * `kind: "active_view"` must move the rendered active highlight — the
   * frontend is NOT authoritative for `active_view`, so the event must be
   * applied, and `ViewsProvider` must read this window's slice.
   */
  it("applies the backend active_view event: the active highlight moves", async () => {
    renderLeftNavLoop();

    const viewOne = await screen.findByRole("button", { name: "View 1" });
    const viewTwo = await screen.findByRole("button", { name: "View 2" });

    // Initial state from `get_ui_state`: v1 is active.
    expect(viewOne.className).toContain(ACTIVE_CLASS);
    expect(viewTwo.className).not.toContain(ACTIVE_CLASS);

    // The user clicks View 2; the backend records the switch and answers
    // with the full per-window snapshot tagged `active_view`.
    fireEvent.click(viewTwo);
    await act(async () => {});
    emitTauriEvent(UI_STATE_CHANGED_EVENT, {
      kind: "active_view",
      state: uiStateSnapshot("v2"),
    });

    expect(viewTwo.className).toContain(ACTIVE_CLASS);
    expect(viewOne.className).not.toContain(ACTIVE_CLASS);
  });
});
