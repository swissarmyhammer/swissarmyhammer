/**
 * Integration tests for view selection through `ViewsContainer` (card
 * 01KTED8XDX4728QR4WT9EZ0WRF — "Remove the view.switch:${id} client
 * indirection").
 *
 * Unlike `views-container.test.tsx` (which mocks `LeftNav` and
 * `views-context`), these tests mount the REAL `ViewsContainer` →
 * `ViewsProvider` → `LeftNav` stack so they pin the production behavior:
 *
 * 1. **Canonical dispatch** — selecting a view dispatches the canonical
 *    `view.set` command with the view id in `args.view_id`. There is no
 *    `view.switch:${id}` hop: the id never existed backend-side (the
 *    dispatcher rewrite was retired in 01KPZMXXEXKVE3RNPA4XJP0105) and the
 *    client-minted indirection was removed by this card.
 *
 * 2. **No minted ids** — no `view.switch:*` command id is registered
 *    anywhere in the scope chain. The per-view scope bookkeeping lives in
 *    presentation (`LeftNav`'s `view:{id}` scope moniker), not in a
 *    client-side command id.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { fireEvent } from "@testing-library/react";
import { useContext } from "react";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { ViewDef } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Mocks — Tauri APIs must be mocked before importing components that pull
// them in transitively.
// ---------------------------------------------------------------------------

const { mockInvoke } = vi.hoisted(() => {
  const mockInvoke = vi.fn(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (_cmd: string, _args?: any): Promise<unknown> => Promise.resolve(null),
  );
  return { mockInvoke };
});

vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...(args as [string, unknown])),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
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
import { ViewsContainer } from "./views-container";
import { UIStateProvider } from "@/lib/ui-state-context";
import {
  CommandScopeProvider,
  CommandScopeContext,
  collectAvailableCommands,
} from "@/lib/command-scope";
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

/**
 * Probe rendered as a `ViewsContainer` child: collects every command id
 * visible through the scope chain at the children's position.
 */
function ScopeIdsProbe() {
  const scope = useContext(CommandScopeContext);
  const ids = collectAvailableCommands(scope).map((e) => e.command.id);
  return <span data-testid="scope-command-ids">{ids.join(",")}</span>;
}

/**
 * Render the REAL ViewsContainer (ViewsProvider + LeftNav) inside the
 * production-shaped provider nest: the spatial stack, a `window:main`
 * command scope (what `WindowContainer` provides in `App.tsx`), and the
 * real UIState provider.
 */
function renderViewsContainer() {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <CommandScopeProvider moniker="window:main">
          <UIStateProvider>
            <TooltipProvider delayDuration={100}>
              <ViewsContainer>
                <ScopeIdsProbe />
              </ViewsContainer>
            </TooltipProvider>
          </UIStateProvider>
        </CommandScopeProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

describe("ViewsContainer — view selection dispatches canonical view.set", () => {
  beforeEach(() => {
    vi.clearAllMocks();
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

  it("selecting a view dispatches view.set with the view id in args", async () => {
    renderViewsContainer();

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
    };
    expect(payload.args).toEqual({ view_id: "v2" });
  });

  it("registers no view.switch:* command id anywhere in the scope chain", async () => {
    renderViewsContainer();

    // Wait until the views list has loaded and rendered — the removed
    // indirection minted its ids exactly when views were present.
    await screen.findByRole("button", { name: "View 2" });

    const ids = screen.getByTestId("scope-command-ids").textContent!;
    const minted = ids.split(",").filter((id) => id.startsWith("view.switch:"));
    expect(minted).toEqual([]);
  });
});
