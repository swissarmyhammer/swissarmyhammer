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
 *    a `ui-state-changed` event (`kind: "active_view"`, full per-window
 *    snapshot). The frontend must apply it (NOT treat `active_view` as a
 *    frontend-authoritative kind), `ViewsProvider` must read THIS window's
 *    `active_view_id` slice, and the left-nav highlight must move.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { ViewDef } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Mocks — Tauri APIs must be mocked before importing components that pull
// them in transitively. `listeners` records every `listen(event, cb)`
// registration so tests can fire backend events by name.
// ---------------------------------------------------------------------------

const { mockInvoke, listeners } = vi.hoisted(() => {
  type Listener = (event: { payload: unknown }) => void;
  const listeners = new Map<string, Listener[]>();
  const mockInvoke = vi.fn(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (_cmd: string, _args?: any): Promise<unknown> => Promise.resolve(null),
  );
  return { mockInvoke, listeners };
});

vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...(args as [string, unknown])),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (event: string, cb: (event: { payload: unknown }) => void) => {
    const list = listeners.get(event) ?? [];
    list.push(cb);
    listeners.set(event, list);
    return Promise.resolve(() => {
      const cur = listeners.get(event) ?? [];
      listeners.set(
        event,
        cur.filter((c) => c !== cb),
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
import { UIStateProvider } from "@/lib/ui-state-context";
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
function uiStateSnapshot(activeViewId: string) {
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
function emitTauriEvent(event: string, payload: unknown) {
  act(() => {
    listeners.get(event)?.forEach((cb) => cb({ payload }));
  });
}

/**
 * Render LeftNav inside the production-shaped provider nest: the spatial
 * stack, a `window:main` command scope (what `WindowContainer` provides in
 * `App.tsx`), and the REAL UIState + Views providers.
 */
function renderLeftNavLoop() {
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
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (cmd: string, _args?: any): Promise<unknown> => {
        if (cmd === "get_ui_state") {
          return Promise.resolve(uiStateSnapshot("v1"));
        }
        if (cmd === "list_views") {
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
   * Consumer loop: a backend `ui-state-changed` event with
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
    emitTauriEvent("ui-state-changed", {
      kind: "active_view",
      state: uiStateSnapshot("v2"),
    });

    expect(viewTwo.className).toContain(ACTIVE_CLASS);
    expect(viewOne.className).not.toContain(ACTIVE_CLASS);
  });
});
