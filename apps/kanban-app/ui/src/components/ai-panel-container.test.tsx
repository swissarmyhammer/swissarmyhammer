/**
 * Component tests for {@link AiPanelContainer}.
 *
 * `AiPanelContainer` is the Container counterpart to the `AiPanel` View (see
 * `ARCHITECTURE.md` Container/View separation): it owns the backend seams the
 * View only renders — model enumeration (`ai_list_models`), the per-board
 * open/width/model persistence, and the right-docked resizable shell.
 *
 * Per-board state (panel open, draggable width, selected model) is persisted
 * in plain `localStorage`, keyed by the active board path — webview-local
 * per-board persistence, exactly as `quick-capture.tsx` persists its last
 * board. There is no backend store involved. These tests pin three contracts
 * the task requires:
 *
 *   - the panel collapses/expands and the collapsed state survives a remount
 *     (read back from the persisted `localStorage` snapshot);
 *   - dragging the left-edge handle updates the width and persists it;
 *   - the quick-capture window never renders the panel.
 *
 * Browser project (`*.test.tsx`) — `AiPanel` and its AI Elements children
 * mount in real Chromium. The `ai_list_models` backend call is mocked; the
 * conversation transport is never exercised here (the View's own wiring is
 * covered by `ai-panel.test.tsx`).
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  act,
  fireEvent,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { userEvent } from "vitest/browser";
import { renderInAct } from "@/test/act-render";
import type { ConversationConnect } from "@/ai/conversation";
import type { AiModel, AiPanelConnectFactory } from "./ai-panel";
import {
  AiPanelContainer,
  AI_PANEL_DEFAULT_WIDTH,
  aiPanelStateStorageKey,
} from "./ai-panel-container";
import { ActiveBoardPathProvider } from "@/lib/command-scope";
import {
  resetAiCommandsForTest,
  triggerAiFocus,
  triggerAiToggle,
} from "@/ai/commands";

// ---------------------------------------------------------------------------
// Tauri API mocks.
//
// The container's own backend seam is `ai_list_models` (via `invoke`). The
// hosted `AiPanel` View now wires its controls into the spatial-nav graph
// (`FocusScope` / `Pressable`), so its module graph transitively imports
// `@tauri-apps/api/event` / `@tauri-apps/api/window` and `@tauri-apps/plugin-log`.
// Those are stubbed here so the real modules never load — the real
// `@tauri-apps/api/event` reaches back into `core` for `transformCallback`,
// which the `core` mock below intentionally does not provide. The container
// itself mounts no `<FocusLayer>`, so the panel's spatial primitives take
// their no-layer fallback path; these mocks just keep the import graph clean.
// ---------------------------------------------------------------------------

const mockInvoke = vi.hoisted(() =>
  vi.fn(async (_cmd: string, _args?: unknown): Promise<unknown> => undefined),
);

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...(args as [string, unknown?])),
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(() => Promise.resolve()),
  listen: vi.fn(() => Promise.resolve(() => {})),
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
// Fixtures
// ---------------------------------------------------------------------------

/** The Claude Code model `ai_list_models` returns in these tests. */
const MODELS: AiModel[] = [
  {
    id: "claude-code",
    label: "Claude Code",
    kind: "claude-code",
    available: true,
    hint: "Claude Code CLI: /usr/local/bin/claude",
  },
];

/** A no-op connect factory — the conversation transport is not exercised here. */
const noopConnect: AiPanelConnectFactory = () => {
  const connect: ConversationConnect = async () => {
    throw new Error("connect must not be called in container tests");
  };
  return connect;
};

const BOARD = "/tmp/board-a";

/**
 * Force the viewport width so the resize clamp upper bound is
 * `min(MAX_PANEL_WIDTH, 0.85 * viewport)` — wide enough that a 540 px target
 * is not clipped. Mirrors `inspector-resize.browser.test.tsx`.
 */
function setViewportWidth(px: number) {
  Object.defineProperty(window, "innerWidth", {
    configurable: true,
    writable: true,
    value: px,
  });
}

/**
 * Render `AiPanelContainer` inside an `ActiveBoardPathProvider` so the
 * container resolves the per-board persistence key, mirroring the production
 * tree where `WindowContainer` provides the active board path.
 */
function renderContainer(
  props: Partial<React.ComponentProps<typeof AiPanelContainer>> = {},
  boardPath: string | undefined = BOARD,
) {
  return renderInAct(
    <ActiveBoardPathProvider value={boardPath}>
      <AiPanelContainer createConnect={noopConnect} {...props} />
    </ActiveBoardPathProvider>,
  );
}

describe("AiPanelContainer", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "ai_list_models") return MODELS;
      return undefined;
    });
    localStorage.clear();
    setViewportWidth(1600); // upper resize clamp = 800
    resetAiCommandsForTest();
  });

  it("renders AiPanel right-docked with the model selector", async () => {
    await renderContainer();

    // The panel shell is present and docked to the right edge.
    const panel = await screen.findByTestId("ai-panel-container");
    expect(panel).not.toBeNull();
    // The AiPanel View mounts inside it.
    await waitFor(() => {
      expect(document.querySelector("[data-slot='ai-panel']")).not.toBeNull();
    });
    // The selector is enabled once `ai_list_models` resolves — opening it
    // surfaces the fetched Claude Code entry.
    const selector = await waitFor(() => {
      const btn = screen.getByRole("button", { name: /select a model/i });
      expect(btn).not.toBeDisabled();
      return btn;
    });
    await act(async () => {
      await userEvent.click(selector);
    });
    const menu = await screen.findByRole("menu");
    expect(
      within(menu).getByRole("menuitem", { name: /claude code/i }),
    ).not.toBeNull();
  });

  it("collapses and expands; the collapsed state persists across a remount", async () => {
    const { unmount } = await renderContainer();

    // Open by default — the panel body is visible.
    await screen.findByTestId("ai-panel-container");
    expect(document.querySelector("[data-slot='ai-panel']")).not.toBeNull();

    // Collapse via the toggle control.
    const toggle = screen.getByRole("button", { name: /collapse ai panel/i });
    await act(async () => {
      fireEvent.click(toggle);
    });

    // The panel body is gone once collapsed.
    await waitFor(() => {
      expect(document.querySelector("[data-slot='ai-panel']")).toBeNull();
    });

    // The collapsed state was persisted per board.
    const stored = JSON.parse(
      localStorage.getItem(aiPanelStateStorageKey(BOARD))!,
    );
    expect(stored.open).toBe(false);

    // Remount — the container reads the persisted state back and stays collapsed.
    unmount();
    await renderContainer();
    await screen.findByTestId("ai-panel-container");
    expect(document.querySelector("[data-slot='ai-panel']")).toBeNull();

    // Expanding again persists `open: true`.
    const expand = screen.getByRole("button", { name: /expand ai panel/i });
    await act(async () => {
      fireEvent.click(expand);
    });
    await waitFor(() => {
      expect(document.querySelector("[data-slot='ai-panel']")).not.toBeNull();
    });
    const reopened = JSON.parse(
      localStorage.getItem(aiPanelStateStorageKey(BOARD))!,
    );
    expect(reopened.open).toBe(true);
  });

  it("dragging the resize handle updates the width and persists it", async () => {
    await renderContainer();
    const panel = await screen.findByTestId("ai-panel-container");

    // The panel starts at the default width.
    expect(panel.style.width).toBe(`${AI_PANEL_DEFAULT_WIDTH}px`);

    const handle = document.querySelector(
      "[data-ai-panel-resize-handle]",
    ) as HTMLElement;
    expect(handle, "drag handle must exist").not.toBeNull();

    // Drag the LEFT edge left by 120 px → the panel grows by 120 px.
    const startX = 1180;
    const endX = startX - 120;
    act(() => {
      fireEvent.mouseDown(handle, { clientX: startX, button: 0 });
    });
    act(() => {
      fireEvent.mouseMove(window, { clientX: endX });
    });

    // Live width during the drag — no persistence round-trip yet.
    const widened = AI_PANEL_DEFAULT_WIDTH + 120;
    expect(panel.style.width).toBe(`${widened}px`);

    act(() => {
      fireEvent.mouseUp(window, { clientX: endX });
    });

    // On release the width is persisted per board.
    await waitFor(() => {
      const stored = JSON.parse(
        localStorage.getItem(aiPanelStateStorageKey(BOARD))!,
      );
      expect(stored.width).toBe(widened);
    });

    // The persisted width is read back on a fresh mount.
    await renderContainer();
    await waitFor(() => {
      const panels = screen.getAllByTestId("ai-panel-container");
      expect(panels[panels.length - 1].style.width).toBe(`${widened}px`);
    });
  });

  it("does not render the panel in the quick-capture window", async () => {
    await renderContainer({ isQuickCapture: true });

    // No panel shell, no AiPanel body — the quick-capture window is inert.
    expect(screen.queryByTestId("ai-panel-container")).toBeNull();
    expect(document.querySelector("[data-slot='ai-panel']")).toBeNull();
    // The container must not even reach for the model list.
    expect(mockInvoke).not.toHaveBeenCalledWith("ai_list_models");
  });

  it("the ai.toggle command handler flips the panel open-state", async () => {
    await renderContainer();

    // Open by default — the panel body is visible.
    await screen.findByTestId("ai-panel-container");
    expect(document.querySelector("[data-slot='ai-panel']")).not.toBeNull();

    // The container registered its `toggle` handler into the AI command
    // registry; firing it (as the window-layer `ai.toggle` command does)
    // collapses the panel.
    await act(async () => {
      triggerAiToggle();
    });
    await waitFor(() => {
      expect(document.querySelector("[data-slot='ai-panel']")).toBeNull();
    });
    // The collapsed state is persisted per board, exactly like the in-header
    // toggle control.
    expect(
      JSON.parse(localStorage.getItem(aiPanelStateStorageKey(BOARD))!).open,
    ).toBe(false);

    // Firing it again expands the panel back.
    await act(async () => {
      triggerAiToggle();
    });
    await waitFor(() => {
      expect(document.querySelector("[data-slot='ai-panel']")).not.toBeNull();
    });
  });

  it("the ai.focus command handler expands a collapsed panel and focuses the prompt", async () => {
    // Seed the board with a chosen model (so the prompt textarea is enabled
    // and focusable) and collapsed (so `ai.focus` must expand it first).
    localStorage.setItem(
      aiPanelStateStorageKey(BOARD),
      JSON.stringify({ open: false, modelId: "claude-code" }),
    );
    await renderContainer();
    await screen.findByTestId("ai-panel-container");
    expect(document.querySelector("[data-slot='ai-panel']")).toBeNull();

    // Firing the registered `ai.focus` handler expands the panel and moves
    // focus into the prompt editor — the AI composer's CM6 content DOM,
    // located by its `role="textbox"` + accessible label.
    await act(async () => {
      triggerAiFocus();
    });
    await waitFor(() => {
      expect(document.querySelector("[data-slot='ai-panel']")).not.toBeNull();
    });
    await waitFor(() => {
      const input = document.querySelector(
        "[data-slot='ai-panel'] [role='textbox'][aria-label='Message the AI agent']",
      );
      expect(input).not.toBeNull();
      // The CM6 content DOM is editable (not `contenteditable="false"`) once
      // a model is selected.
      expect(input!.getAttribute("contenteditable")).toBe("true");
      expect(document.activeElement).toBe(input);
    });
  });

  it("persists and reapplies the per-board model choice", async () => {
    await renderContainer();
    await screen.findByTestId("ai-panel-container");

    // No model is chosen yet — nothing persisted.
    expect(localStorage.getItem(aiPanelStateStorageKey(BOARD))).toBeNull();

    // The container persists the choice when the View reports one. Drive
    // `onSelectModel` through the selector dropdown: the trigger reads
    // "Select a model" until a model is picked.
    const selector = await waitFor(() => {
      const btn = screen.getByRole("button", { name: /select a model/i });
      expect(btn).not.toBeDisabled();
      return btn;
    });
    await act(async () => {
      await userEvent.click(selector);
    });
    const menu = await screen.findByRole("menu");
    await act(async () => {
      await userEvent.click(
        within(menu).getByRole("menuitem", { name: /claude code/i }),
      );
    });

    // The Container persisted the per-board model id.
    await waitFor(() => {
      const stored = JSON.parse(
        localStorage.getItem(aiPanelStateStorageKey(BOARD))!,
      );
      expect(stored.modelId).toBe("claude-code");
    });

    // A fresh mount reapplies the persisted model — the selector now shows it.
    await renderContainer();
    await waitFor(() => {
      const triggers = screen.getAllByRole("button", { name: /claude code/i });
      expect(triggers.length).toBeGreaterThan(0);
    });
  });
});
