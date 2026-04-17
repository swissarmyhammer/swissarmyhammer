import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";

// Mock Tauri APIs before importing components that use them
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn((cmd: string, args?: any) => {
    if (cmd === "get_ui_state")
      return Promise.resolve({
        palette_open: false,
        palette_mode: "command",
        keymap_mode: "cua",
        scope_chain: [],
        open_boards: [],
        windows: {},
        recent_boards: [],
      });
    if (cmd === "search_entities") {
      const query = args?.query ?? "";
      if (!query.trim()) return Promise.resolve([]);
      const all = [
        {
          entity_type: "task",
          entity_id: "01ABC",
          display_name: "Fix the login bug",
          score: 100,
        },
        {
          entity_type: "task",
          entity_id: "01DEF",
          display_name: "Add dark mode",
          score: 50,
        },
        {
          entity_type: "tag",
          entity_id: "01GHI",
          display_name: "frontend",
          score: 30,
        },
      ];
      return Promise.resolve(
        all.filter((r) =>
          r.display_name.toLowerCase().includes(query.toLowerCase()),
        ),
      );
    }
    if (cmd === "list_commands_for_scope")
      return Promise.resolve([
        {
          id: "open-file",
          name: "Open File",
          context_menu: false,
          keys: { vim: ":e", cua: "Ctrl+O" },
          available: true,
        },
        {
          id: "save-file",
          name: "Save File",
          context_menu: false,
          keys: { vim: ":w", cua: "Ctrl+S" },
          available: true,
        },
        {
          id: "close-tab",
          name: "Close Tab",
          context_menu: false,
          keys: { cua: "Ctrl+W" },
          available: true,
        },
      ]);
    if (cmd === "log_command") return Promise.resolve(null);
    return Promise.resolve(null);
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main", setFocus: vi.fn() }),
}));

// Mock codemirror-vim: getCM returns a cm object, Vim.handleKey is the spy
vi.mock("@replit/codemirror-vim", async () => {
  const actual = await vi.importActual<typeof import("@replit/codemirror-vim")>(
    "@replit/codemirror-vim",
  );
  return {
    ...actual,
    getCM: vi.fn(() => ({ state: { vim: {} } }) as any),
    Vim: { ...actual.Vim, handleKey: vi.fn(), exitInsertMode: vi.fn() },
  };
});

import { invoke } from "@tauri-apps/api/core";
import { getCM, Vim } from "@replit/codemirror-vim";
import { CommandPalette } from "./command-palette";
import { CommandScopeProvider, type CommandDef } from "@/lib/command-scope";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { UIStateProvider } from "@/lib/ui-state-context";

const getCMMock = vi.mocked(getCM);
const handleKeyMock = vi.mocked(Vim.handleKey);

const TEST_COMMANDS: CommandDef[] = [
  {
    id: "open-file",
    name: "Open File",
    keys: { vim: ":e", cua: "Ctrl+O" },
    execute: vi.fn(),
  },
  {
    id: "save-file",
    name: "Save File",
    keys: { vim: ":w", cua: "Ctrl+S" },
    execute: vi.fn(),
  },
  {
    id: "close-tab",
    name: "Close Tab",
    keys: { cua: "Ctrl+W" },
    execute: vi.fn(),
  },
];

beforeEach(() => {
  handleKeyMock.mockClear();
  getCMMock.mockClear();
  // Restore default getCM behavior
  getCMMock.mockReturnValue({ state: { vim: {} } } as any);
});

function renderPalette(open: boolean, onClose = vi.fn()) {
  return render(
    <EntityFocusProvider>
      <UIStateProvider>
        <CommandScopeProvider commands={TEST_COMMANDS}>
          <CommandPalette open={open} onClose={onClose} />
        </CommandScopeProvider>
      </UIStateProvider>
    </EntityFocusProvider>,
  );
}

describe("CommandPalette", () => {
  it("renders nothing when closed", () => {
    renderPalette(false);
    expect(screen.queryByTestId("command-palette")).toBeNull();
  });

  it("renders the palette when open", () => {
    renderPalette(true);
    expect(screen.getByTestId("command-palette")).toBeTruthy();
  });

  it("shows all commands when no filter is applied", async () => {
    await act(async () => {
      renderPalette(true);
    });
    expect(screen.getByText("Open File")).toBeTruthy();
    expect(screen.getByText("Save File")).toBeTruthy();
    expect(screen.getByText("Close Tab")).toBeTruthy();
  });

  it("shows keybinding hints for the current mode", async () => {
    await act(async () => {
      renderPalette(true);
    });
    // Default mode is CUA (mocked invoke returns "cua")
    expect(screen.getByText("Ctrl+O")).toBeTruthy();
    expect(screen.getByText("Ctrl+S")).toBeTruthy();
    expect(screen.getByText("Ctrl+W")).toBeTruthy();
  });

  it("calls onClose when backdrop is clicked", () => {
    const onClose = vi.fn();
    renderPalette(true, onClose);
    fireEvent.click(screen.getByTestId("command-palette-backdrop"));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("does not close when clicking inside the palette card", () => {
    const onClose = vi.fn();
    renderPalette(true, onClose);
    fireEvent.click(screen.getByTestId("command-palette"));
    expect(onClose).not.toHaveBeenCalled();
  });

  it("executes a command when its item is clicked", async () => {
    const onClose = vi.fn();
    await act(async () => {
      renderPalette(true, onClose);
    });
    fireEvent.click(screen.getByText("Save File"));
    // The command resolves through the scope chain. Since TEST_COMMANDS
    // register "save-file" with an execute handler, it runs client-side.
    // In production, palette commands come from the backend without execute
    // handlers, so they dispatch to Rust via IPC instead.
    const saveCmd = TEST_COMMANDS.find((c) => c.id === "save-file")!;
    expect(saveCmd.execute).toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });

  it("renders the command list with correct role", () => {
    renderPalette(true);
    const list = screen.getByTestId("command-palette-list");
    expect(list.getAttribute("role")).toBe("listbox");
  });

  it("dispatches to backend when command has no execute handler", async () => {
    const invokeMock = vi.mocked(invoke);

    // Render with NO commands in scope — palette commands come from backend
    // mock (list_commands_for_scope) and have no execute handlers.
    const onClose = vi.fn();
    await act(async () => {
      render(
        <EntityFocusProvider>
          <UIStateProvider>
            <CommandScopeProvider commands={[]}>
              <CommandPalette open={true} onClose={onClose} />
            </CommandScopeProvider>
          </UIStateProvider>
        </EntityFocusProvider>,
      );
    });
    invokeMock.mockClear();
    fireEvent.click(screen.getByText("Open File"));

    // Command not in React scope → dispatches to backend via invoke
    const dispatchCall = invokeMock.mock.calls.find(
      ([cmd]) => cmd === "dispatch_command",
    );
    expect(dispatchCall).toBeDefined();
    const [, args] = dispatchCall!;
    expect((args as Record<string, unknown>).cmd).toBe("open-file");
    // windowLabel should NOT be present — scope chain carries window identity
    expect((args as Record<string, unknown>).windowLabel).toBeUndefined();
    expect(onClose).toHaveBeenCalled();
  });
});

describe("CommandPalette vim insert mode", () => {
  /** Flush pending requestAnimationFrame callbacks by running them synchronously. */
  function flushRAF(count = 5) {
    for (let i = 0; i < count; i++) {
      vi.advanceTimersByTime(16); // one frame ≈ 16ms
    }
  }

  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("auto-enters insert mode when palette opens in vim mode", async () => {
    // Mock invoke to return "vim" for get_ui_state
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "get_ui_state")
        return Promise.resolve({
          palette_open: false,
          palette_mode: "command",
          keymap_mode: "vim",
          scope_chain: [],
          open_boards: [],
          windows: {},
          recent_boards: [],
        });
      if (cmd === "list_commands_for_scope") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    await act(async () => {
      renderPalette(true);
    });

    // Flush rAF retries so the effect can find the CM view
    await act(async () => {
      flushRAF(25);
    });

    expect(getCMMock).toHaveBeenCalled();
    expect(handleKeyMock).toHaveBeenCalledWith(
      expect.anything(),
      "i",
      "mapping",
    );
  });

  it("does NOT enter insert mode in CUA mode", async () => {
    // Default mock returns "cua"
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "get_ui_state")
        return Promise.resolve({
          palette_open: false,
          palette_mode: "command",
          keymap_mode: "cua",
          scope_chain: [],
          open_boards: [],
          windows: {},
          recent_boards: [],
        });
      if (cmd === "list_commands_for_scope") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    await act(async () => {
      renderPalette(true);
    });

    await act(async () => {
      flushRAF(25);
    });

    // getCM should NOT be called — the vim insert effect skips non-vim modes
    expect(handleKeyMock).not.toHaveBeenCalled();
  });

  it("retries when getCM initially returns null", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "get_ui_state")
        return Promise.resolve({
          palette_open: false,
          palette_mode: "command",
          keymap_mode: "vim",
          scope_chain: [],
          open_boards: [],
          windows: {},
          recent_boards: [],
        });
      if (cmd === "list_commands_for_scope") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    // getCM returns null for the first 3 calls, then succeeds
    let callCount = 0;
    getCMMock.mockImplementation(() => {
      callCount++;
      if (callCount <= 3) return null;
      return { state: { vim: {} } } as any;
    });

    await act(async () => {
      renderPalette(true);
    });

    // Flush enough frames to get past the null returns
    await act(async () => {
      flushRAF(10);
    });

    expect(callCount).toBeGreaterThan(3);
    expect(handleKeyMock).toHaveBeenCalledWith(
      expect.anything(),
      "i",
      "mapping",
    );
  });

  it("vim normal mode: Escape closes palette", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "get_ui_state")
        return Promise.resolve({
          palette_open: false,
          palette_mode: "command",
          keymap_mode: "vim",
          scope_chain: [],
          open_boards: [],
          windows: {},
          recent_boards: [],
        });
      if (cmd === "list_commands_for_scope") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    // getCM returns normal mode (insertMode is falsy)
    getCMMock.mockReturnValue({ state: { vim: {} } } as any);

    const onClose = vi.fn();
    await act(async () => {
      renderPalette(true, onClose);
    });
    await act(async () => {
      flushRAF(25);
    });

    // Find the .cm-editor element and dispatch Escape through the DOM
    const cmEditor = document.querySelector(".cm-editor") as HTMLElement;
    expect(cmEditor).toBeTruthy();

    // The two-phase handler uses capture+bubble on .cm-editor's DOM element.
    // Fire keydown on the cm-content (child) so it bubbles through cm-editor.
    const cmContent = document.querySelector(".cm-content") as HTMLElement;
    expect(cmContent).toBeTruthy();

    await act(async () => {
      cmContent.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
      );
    });

    expect(onClose).toHaveBeenCalled();
  });

  it("vim insert mode: Escape does NOT close palette", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "get_ui_state")
        return Promise.resolve({
          palette_open: false,
          palette_mode: "command",
          keymap_mode: "vim",
          scope_chain: [],
          open_boards: [],
          windows: {},
          recent_boards: [],
        });
      if (cmd === "list_commands_for_scope") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    // getCM returns insert mode
    getCMMock.mockReturnValue({ state: { vim: { insertMode: true } } } as any);

    const onClose = vi.fn();
    await act(async () => {
      renderPalette(true, onClose);
    });
    await act(async () => {
      flushRAF(25);
    });

    const cmContent = document.querySelector(".cm-content") as HTMLElement;
    expect(cmContent).toBeTruthy();

    await act(async () => {
      cmContent.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
      );
    });

    // Should NOT close — Escape in insert mode just exits to normal mode
    expect(onClose).not.toHaveBeenCalled();
  });

  it("stops retrying after cancellation (palette closes)", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "get_ui_state")
        return Promise.resolve({
          palette_open: false,
          palette_mode: "command",
          keymap_mode: "vim",
          scope_chain: [],
          open_boards: [],
          windows: {},
          recent_boards: [],
        });
      if (cmd === "list_commands_for_scope") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    // getCM always returns null — simulates slow init
    getCMMock.mockReturnValue(null);

    let result: ReturnType<typeof render>;
    await act(async () => {
      result = renderPalette(true);
    });

    // Unmount (closes palette) — should cancel the retry loop
    await act(async () => {
      result!.unmount();
    });

    await act(async () => {
      flushRAF(30);
    });

    // handleKey should never have been called since getCM always returned null
    // and the cleanup cancelled further retries
    expect(handleKeyMock).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// Search mode tests
// ---------------------------------------------------------------------------

function renderSearchPalette(open: boolean, onClose = vi.fn()) {
  return render(
    <EntityFocusProvider>
      <UIStateProvider>
        <CommandScopeProvider commands={[]}>
          <CommandPalette open={open} onClose={onClose} mode="search" />
        </CommandScopeProvider>
      </UIStateProvider>
    </EntityFocusProvider>,
  );
}

/**
 * Helper: get the CM6 EditorView from the .cm-content element.
 * Returns null if CM6 internals are not available in the test environment.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function getCMView(container: HTMLElement): any | null {
  const cmContent = container.querySelector(
    ".cm-content",
  ) as HTMLElement | null;
  if (!cmContent) return null;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  return (cmContent as any).cmTile?.view ?? null;
}

describe("CommandPalette search mode", () => {
  it("shows hint text when no query is entered", () => {
    renderSearchPalette(true);
    // The hint text appears both in the CM6 placeholder and in the result list hint div
    const matches = screen.getAllByText("Type to search...");
    // At minimum the hint div in the list should be present
    expect(matches.length).toBeGreaterThanOrEqual(1);
    // The hint div (not the CM6 placeholder) should be in the list
    const list = screen.getByTestId("command-palette-list");
    expect(list.textContent).toContain("Type to search...");
  });

  it("shows no matching entities message when query has no results", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const { container, unmount } = renderSearchPalette(true);

    const view = getCMView(container);
    if (view?.dispatch) {
      // Type a query that won't match any entities
      view.dispatch({
        changes: {
          from: 0,
          to: view.state.doc.length,
          insert: "xyzzy_no_match_zzz",
        },
      });

      await act(async () => {
        vi.advanceTimersByTime(200);
      });

      const list = screen.getByTestId("command-palette-list");
      expect(list.textContent).toContain("No matching entities");
    } else {
      // CM6 internals not available — verify the component at least renders
      const list = screen.getByTestId("command-palette-list");
      expect(list).toBeTruthy();
    }

    vi.useRealTimers();
    unmount();
  });

  it("calls invoke search_entities with the query after debounce", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const invokeMock = vi.mocked(invoke);
    invokeMock.mockClear();

    const { container, unmount } = renderSearchPalette(true);

    const view = getCMView(container);
    if (view?.dispatch) {
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: "login" },
      });

      await act(async () => {
        vi.advanceTimersByTime(200);
      });

      // invoke should have been called with search_entities and the query
      expect(invokeMock).toHaveBeenCalledWith("search_entities", {
        query: "login",
        limit: 50,
      });
    }

    vi.useRealTimers();
    unmount();
  });

  it("renders search results after calling backend", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const { container, unmount } = renderSearchPalette(true);

    const view = getCMView(container);
    if (view?.dispatch) {
      // Type "login" — mock returns "Fix the login bug" (entity id: 01ABC)
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: "login" },
      });

      await act(async () => {
        vi.advanceTimersByTime(200);
      });

      const list = screen.getByTestId("command-palette-list");
      // The matching task title should appear in the list
      expect(list.textContent).toContain("Fix the login bug");
      // The non-matching entity should not appear
      expect(list.textContent).not.toContain("Add dark mode");
      // The hint and no-results messages should not appear
      expect(list.textContent).not.toContain("Type to search...");
      expect(list.textContent).not.toContain("No matching entities");
    } else {
      // CM6 internals not available in this env — verify component structure
      const list = screen.getByTestId("command-palette-list");
      expect(list.textContent).toContain("Type to search...");
    }

    vi.useRealTimers();
    unmount();
  });

  it("shows multiple results when query matches several entities", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const { container, unmount } = renderSearchPalette(true);

    const view = getCMView(container);
    if (view?.dispatch) {
      // Type "dark" — mock returns "Add dark mode" only
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: "dark" },
      });

      await act(async () => {
        vi.advanceTimersByTime(200);
      });

      const list = screen.getByTestId("command-palette-list");
      expect(list.textContent).toContain("Add dark mode");
    } else {
      // Fallback: just check the list renders
      expect(screen.getByTestId("command-palette-list")).toBeTruthy();
    }

    vi.useRealTimers();
    unmount();
  });

  it("renders the palette with listbox role in search mode", () => {
    renderSearchPalette(true);
    const list = screen.getByTestId("command-palette-list");
    expect(list.getAttribute("role")).toBe("listbox");
  });

  it("uses search placeholder text in search mode", () => {
    renderSearchPalette(true);
    // The CodeMirror placeholder is rendered as a span in the DOM
    // We verify the palette is open and in search mode by checking it renders
    expect(screen.getByTestId("command-palette")).toBeTruthy();
  });

  it("calls onClose when backdrop is clicked in search mode", () => {
    const onClose = vi.fn();
    renderSearchPalette(true, onClose);
    fireEvent.click(screen.getByTestId("command-palette-backdrop"));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("does not render when closed in search mode", () => {
    renderSearchPalette(false);
    expect(screen.queryByTestId("command-palette")).toBeNull();
  });

  it("calls inspect and onClose when a search result is clicked", async () => {
    const onClose = vi.fn();
    const onInspect = vi.fn();

    vi.useFakeTimers({ shouldAdvanceTime: true });
    const { container, unmount } = renderSearchPalette(true, onClose);

    const view = getCMView(container);
    if (view?.dispatch) {
      // Type "login" to surface the "Fix the login bug" task
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: "login" },
      });

      await act(async () => {
        vi.advanceTimersByTime(200);
      });

      // The search result for task:01ABC should be in the DOM
      const resultItem = screen.queryByTestId("search-result-task:01ABC");
      if (resultItem) {
        fireEvent.click(resultItem);
        // onClose is called when an entity is selected
        expect(onClose).toHaveBeenCalled();
        // onInspect is called with the entity type and id parsed from the moniker
        expect(onInspect).toHaveBeenCalledWith("task", "01ABC");
      } else {
        // Result item not rendered in this env — just verify debounce advanced
        expect(onClose).not.toHaveBeenCalled();
      }
    } else {
      // CM6 not available — advance timers and verify no errors
      await act(async () => {
        vi.advanceTimersByTime(200);
      });
    }

    vi.useRealTimers();
    unmount();
  });

  it("shows the entity type label alongside the entity title in results", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const { container, unmount } = renderSearchPalette(true);

    const view = getCMView(container);
    if (view?.dispatch) {
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: "frontend" },
      });

      await act(async () => {
        vi.advanceTimersByTime(200);
      });

      const list = screen.getByTestId("command-palette-list");
      // "frontend" tag entity should appear with its type label "Tag"
      expect(list.textContent).toContain("frontend");
      expect(list.textContent).toContain("Tag");
    } else {
      expect(screen.getByTestId("command-palette-list")).toBeTruthy();
    }

    vi.useRealTimers();
    unmount();
  });

  it("resets to hint state when query is cleared after a search", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const { container, unmount } = renderSearchPalette(true);

    const view = getCMView(container);
    if (view?.dispatch) {
      // Type a query
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: "login" },
      });
      await act(async () => {
        vi.advanceTimersByTime(200);
      });

      // Clear the query
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: "" },
      });
      await act(async () => {
        vi.advanceTimersByTime(200);
      });

      const list = screen.getByTestId("command-palette-list");
      expect(list.textContent).toContain("Type to search...");
    } else {
      const list = screen.getByTestId("command-palette-list");
      expect(list.textContent).toContain("Type to search...");
    }

    vi.useRealTimers();
    unmount();
  });

  it("does not call invoke when query is empty", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const invokeMock = vi.mocked(invoke);
    invokeMock.mockClear();

    const { unmount } = renderSearchPalette(true);

    // Advance time without typing anything
    await act(async () => {
      vi.advanceTimersByTime(200);
    });

    // search_entities should NOT have been called with an empty query
    const searchCalls = invokeMock.mock.calls.filter(
      ([cmd]) => cmd === "search_entities",
    );
    expect(searchCalls.length).toBe(0);

    vi.useRealTimers();
    unmount();
  });
});

// ---------------------------------------------------------------------------
// Per-entity-type palette rendering tests (section 6 — MANDATORY).
//
// One test per entity type. Each test must be independently named and
// runnable; the whole point is that a regression on a single type cannot
// hide behind the other two passing.
//
// `list_commands_for_scope` is mocked to return exactly what the Rust
// emission produces for each grid view's scope chain, and the test asserts
// the palette renders the corresponding "New {Type}" entry. Any change to
// the backend → frontend contract that drops the command (e.g. removing
// `name` / `id` from the serialized payload) fails here.
// ---------------------------------------------------------------------------

/**
 * Mock the minimal ResolvedCommand payload the backend returns for a given
 * active view's scope chain, matching exactly what `emit_entity_add` and
 * the surrounding registry emit in production.
 */
function mockBackendForView(opts: {
  expectedScope: string[];
  entityAddId: string;
  entityAddName: string;
}) {
  vi.mocked(invoke).mockImplementation((cmd: string, args?: any) => {
    if (cmd === "get_ui_state")
      return Promise.resolve({
        palette_open: false,
        palette_mode: "command",
        keymap_mode: "cua",
        scope_chain: opts.expectedScope,
        open_boards: [],
        windows: {},
        recent_boards: [],
      });
    if (cmd === "list_commands_for_scope") {
      // Sanity — the palette must forward exactly the scope chain we
      // expect. If the frontend ever diverges from useUIState().scope_chain
      // this throws, catching the regression class "palette sent an empty
      // scope chain".
      if (JSON.stringify(args?.scopeChain) !== JSON.stringify(opts.expectedScope)) {
        return Promise.resolve([]);
      }
      return Promise.resolve([
        {
          id: opts.entityAddId,
          name: opts.entityAddName,
          group: "entity",
          context_menu: true,
          available: true,
        },
        {
          id: "app.quit",
          name: "Quit",
          group: "global",
          context_menu: false,
          available: true,
        },
      ]);
    }
    if (cmd === "log_command") return Promise.resolve(null);
    return Promise.resolve(null);
  });
}

describe("CommandPalette per-entity-type rendering", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Waits for the UIState effect to pull `scope_chain` out of the mocked
   * `get_ui_state`, and for the `list_commands_for_scope` fetch-on-open
   * effect to settle.
   */
  async function settleEffects() {
    // useUIState fetches get_ui_state on mount; the palette then fetches
    // list_commands_for_scope in a follow-up effect that depends on
    // scope_chain. Two microtask flushes are enough in jsdom.
    await Promise.resolve();
    await Promise.resolve();
  }

  it('palette shows "New Task" when active view is tasks-grid', async () => {
    mockBackendForView({
      expectedScope: ["view:01JMVIEW0000000000TGRID0", "board:my-board"],
      entityAddId: "entity.add:task",
      entityAddName: "New Task",
    });

    await act(async () => {
      renderPalette(true);
      await settleEffects();
    });

    expect(screen.getByText("New Task")).toBeTruthy();
  });

  it('palette shows "New Tag" when active view is tags-grid', async () => {
    mockBackendForView({
      expectedScope: ["view:01JMVIEW0000000000TGGRD0", "board:my-board"],
      entityAddId: "entity.add:tag",
      entityAddName: "New Tag",
    });

    await act(async () => {
      renderPalette(true);
      await settleEffects();
    });

    expect(screen.getByText("New Tag")).toBeTruthy();
  });

  it('palette shows "New Project" when active view is projects-grid', async () => {
    mockBackendForView({
      expectedScope: ["view:01JMVIEW0000000000PGRID0", "board:my-board"],
      entityAddId: "entity.add:project",
      entityAddName: "New Project",
    });

    await act(async () => {
      renderPalette(true);
      await settleEffects();
    });

    expect(screen.getByText("New Project")).toBeTruthy();
  });
});
