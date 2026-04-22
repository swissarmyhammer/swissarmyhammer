import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { ViewDef } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Mocks — Tauri APIs must be mocked before importing any components that
// transitively pull them in through context providers.
// ---------------------------------------------------------------------------

/**
 * Records and controls `invoke` calls. Tests assert on the argument list of
 * each call — particularly `list_commands_for_scope` (right-click builds
 * the menu) and `show_context_menu` (the native menu that actually pops up).
 */
const mockInvoke = vi.fn(
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (_cmd: string, _args?: any): Promise<unknown> => Promise.resolve(null),
);

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

// Mutable fixture the mocked views-context hook reads every render so each
// test can install its own views/activeView without re-mocking the module.
let mockViewsValue: {
  views: ViewDef[];
  activeView: ViewDef | null;
  setActiveViewId: (id: string) => void;
  refresh: () => Promise<void>;
} = {
  views: [],
  activeView: null,
  setActiveViewId: vi.fn(),
  refresh: vi.fn(() => Promise.resolve()),
};

vi.mock("@/lib/views-context", () => ({
  useViews: () => mockViewsValue,
}));

// Import after mocks so the mock module bindings are the ones left-nav pulls.
import { LeftNav } from "./left-nav";

/** Renders LeftNav inside the required TooltipProvider. */
function renderLeftNav() {
  return render(
    <TooltipProvider delayDuration={100}>
      <LeftNav />
    </TooltipProvider>,
  );
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const V1: ViewDef = { id: "v1", name: "View 1", kind: "board", icon: "kanban" };
const V2: ViewDef = { id: "v2", name: "View 2", kind: "grid", icon: "table" };

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("LeftNav — right-click context menu", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockViewsValue = {
      views: [V1, V2],
      activeView: V1,
      setActiveViewId: vi.fn(),
      refresh: vi.fn(() => Promise.resolve()),
    };
  });

  /**
   * Right-click on a view button must call `list_commands_for_scope` with
   * that view's moniker in the scope chain. This is what the Rust backend
   * uses to emit only the matching `view.switch:{id}` as a context-menu
   * entry.
   */
  it("right-click on a view button queries commands with that view's scope", () => {
    // First button corresponds to view v1.
    mockInvoke.mockImplementationOnce(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (_cmd: string, _args?: any) => Promise.resolve([]),
    );

    renderLeftNav();

    const buttons = screen.getAllByRole("button");
    // The two view icon buttons are the only buttons LeftNav renders.
    expect(buttons.length).toBe(2);

    fireEvent.contextMenu(buttons[0]);

    expect(mockInvoke).toHaveBeenCalledWith(
      "list_commands_for_scope",
      expect.objectContaining({
        scopeChain: expect.arrayContaining(["view:v1"]),
        contextMenu: true,
      }),
    );
  });

  /**
   * After the backend returns a single `view.switch:v1` entry, the hook
   * must forward exactly that one item to `show_context_menu`. The full
   * dispatch info (cmd id, name, scope chain) is attached so the native
   * menu can dispatch without a round-trip.
   */
  it("shows native context menu with the backend-supplied Switch to <view> entry", async () => {
    mockInvoke.mockImplementation(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (cmd: string, _args?: any) => {
        if (cmd === "list_commands_for_scope") {
          return Promise.resolve([
            {
              id: "view.switch:v1",
              name: "Switch to View 1",
              group: "view",
              context_menu: true,
              available: true,
            },
          ]);
        }
        return Promise.resolve(null);
      },
    );

    renderLeftNav();

    const buttons = screen.getAllByRole("button");
    fireEvent.contextMenu(buttons[0]);

    // `useContextMenu` kicks off list_commands_for_scope then awaits the
    // promise before calling show_context_menu; flush microtasks so the
    // second invoke has happened by the time we assert.
    await Promise.resolve();
    await Promise.resolve();

    const showCall = mockInvoke.mock.calls.find(
      ([cmd]) => cmd === "show_context_menu",
    );
    expect(showCall).toBeDefined();
    const items = (showCall![1] as { items: unknown[] }).items as Array<{
      cmd: string;
      name: string;
      separator: boolean;
      scope_chain: string[];
    }>;
    expect(items).toHaveLength(1);
    expect(items[0].cmd).toBe("view.switch:v1");
    expect(items[0].name).toBe("Switch to View 1");
    expect(items[0].separator).toBe(false);
    expect(items[0].scope_chain).toEqual(expect.arrayContaining(["view:v1"]));
  });

  /**
   * Left-click still dispatches the view switch through the command pipeline
   * (regression guard: wiring the context-menu handler must not swallow
   * click events on the same button).
   */
  it("left-click dispatches view.switch:{id} through dispatch_command", async () => {
    mockInvoke.mockImplementation(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (_cmd: string, _args?: any) => Promise.resolve(null),
    );

    renderLeftNav();

    const buttons = screen.getAllByRole("button");
    fireEvent.click(buttons[1]);

    await Promise.resolve();
    await Promise.resolve();

    const dispatchCall = mockInvoke.mock.calls.find(
      ([cmd]) => cmd === "dispatch_command",
    );
    expect(dispatchCall).toBeDefined();
    expect((dispatchCall![1] as { cmd: string }).cmd).toBe("view.switch:v2");
  });
});
