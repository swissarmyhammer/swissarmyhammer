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
import type {
  ContentBlock,
  PromptResponse,
  SessionNotification,
} from "@agentclientprotocol/sdk";
import type {
  AcpSession,
  KanbanAcpClient,
  SessionUpdateHandler,
} from "@/ai/acp-client";
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

/**
 * Whether the hosted `AiPanel` body is hidden by the shell.
 *
 * Collapsing the panel must NOT unmount the body — the contract is "still
 * present but hidden". The shell hides it by setting `hidden` on the
 * always-mounted wrapper around `{children}`, which CSS-hides via
 * `display: none`. Walks up from the `data-slot='ai-panel'` element to its
 * nearest ancestor carrying `hidden` (HTML `hidden` attribute or the `hidden`
 * Tailwind class compiled to `display: none`).
 */
function isPanelBodyHidden(panelBody: Element): boolean {
  let node: Element | null = panelBody;
  while (node) {
    const el = node as HTMLElement;
    if (el.hidden) return true;
    if (el.classList.contains("hidden")) return true;
    if (getComputedStyle(el).display === "none") return true;
    node = node.parentElement;
  }
  return false;
}

/**
 * Replay scripted ACP `session/update` notifications during a fake turn.
 *
 * Mirrors the `SessionScript` pattern in `ai-panel.test.tsx`: the test scripts
 * notifications that the panel folds into renderable conversation state, then
 * `prompt` resolves with a `stopReason`.
 */
interface SessionScript {
  /** `session/update` notifications streamed before `prompt` resolves. */
  updates?: SessionNotification["update"][];
  /** The stop reason `prompt` resolves with (default `end_turn`). */
  stopReason?: PromptResponse["stopReason"];
}

/** A controllable fake ACP session with no transport. */
class FakeSession implements AcpSession {
  readonly sessionId = "fake-container-session";
  readonly prompts: ContentBlock[][] = [];
  cancelled = false;

  constructor(
    private readonly onUpdate: SessionUpdateHandler,
    private readonly script: SessionScript,
    /**
     * The board directory the parent factory captured at construction time —
     * the production analogue is `cwd` passed to `newSession`. Exposed so the
     * board-switch regression test can assert the session was started against
     * the new board.
     */
    readonly cwd: string,
  ) {}

  async prompt(prompt: ContentBlock[]): Promise<PromptResponse> {
    this.prompts.push(prompt);
    for (const update of this.script.updates ?? []) {
      await this.onUpdate({ sessionId: this.sessionId, update });
    }
    return { stopReason: this.script.stopReason ?? "end_turn" };
  }

  async cancel(): Promise<void> {
    this.cancelled = true;
  }

  async setMode(): Promise<void> {
    // Unused by these tests.
  }
}

/** A mock {@link AiPanelConnectFactory} plus the seams a test inspects. */
interface MockHarness {
  /** The factory to inject as the container's `createConnect` prop. */
  createConnect: AiPanelConnectFactory;
  /** Every model id the factory was invoked with, in order. */
  connectedModels: () => string[];
  /** Every {@link FakeSession} the fake client started, in order. */
  sessions: () => FakeSession[];
}

/**
 * Build a {@link MockHarness} backed by {@link FakeSession}s.
 *
 * The returned factory captures which model ids the panel connects for and
 * exposes the constructed sessions so the test can assert no fresh connect or
 * session was created across a panel toggle. The factory is bound to a single
 * `boardDir` — production's `useProductionConnect(boardDir)` returns a new
 * factory whenever the active board changes, and the harness mirrors that:
 * board-switch tests build a fresh harness for each board.
 */
function mockHarness(
  script: SessionScript = {},
  boardDir: string = BOARD,
): MockHarness {
  const connectedModels: string[] = [];
  const sessions: FakeSession[] = [];

  const createConnect: AiPanelConnectFactory = (modelId) => {
    connectedModels.push(modelId);
    const connect: ConversationConnect = async (handlers) => {
      const client: KanbanAcpClient = {
        protocolVersion: 1,
        initializeResponse: { protocolVersion: 1, agentCapabilities: {} },
        async startSession(): Promise<AcpSession> {
          const session = new FakeSession(
            handlers.onSessionUpdate,
            script,
            boardDir,
          );
          sessions.push(session);
          return session;
        },
      };
      return client;
    };
    return connect;
  };

  return {
    createConnect,
    connectedModels: () => connectedModels,
    sessions: () => sessions,
  };
}

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

/**
 * Wrap `AiPanelContainer` in an `ActiveBoardPathProvider` for board-switch
 * tests. The wrapper takes the current `boardPath` and `createConnect` so the
 * test can rerender with a different board and a freshly-bound factory in one
 * pass — mirroring the production `useProductionConnect(boardPath)` pattern.
 */
function BoardScopedContainer({
  boardPath,
  createConnect,
}: {
  boardPath: string;
  createConnect: AiPanelConnectFactory;
}) {
  return (
    <ActiveBoardPathProvider value={boardPath}>
      <AiPanelContainer createConnect={createConnect} />
    </ActiveBoardPathProvider>
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
    // The selector — now the AI Elements `PromptInputSelect` in the composer
    // footer — is enabled once `ai_list_models` resolves. With the
    // auto-select default in place, the trigger reads the picked model's
    // label ("Claude Code") rather than the placeholder; opening it still
    // surfaces the fetched Claude Code entry.
    const selector = await waitFor(() => {
      const btn = screen.getByRole("combobox", { name: /claude code/i });
      expect(btn).not.toBeDisabled();
      return btn;
    });
    await act(async () => {
      await userEvent.click(selector);
    });
    const listbox = await screen.findByRole("listbox");
    expect(
      within(listbox).getByRole("option", { name: /claude code/i }),
    ).not.toBeNull();
  });

  it("collapses and expands; the collapsed state persists across a remount", async () => {
    const { unmount } = await renderContainer();

    // Open by default — the panel body is visible.
    await screen.findByTestId("ai-panel-container");
    const initialBody = document.querySelector("[data-slot='ai-panel']");
    expect(initialBody).not.toBeNull();
    expect(isPanelBodyHidden(initialBody!)).toBe(false);

    // Collapse via the toggle control.
    const toggle = screen.getByRole("button", { name: /collapse ai panel/i });
    await act(async () => {
      fireEvent.click(toggle);
    });

    // The panel body stays mounted but is hidden once collapsed — toggling the
    // panel must not unmount the conversation (see "conversation survives a
    // toggle" below).
    await waitFor(() => {
      const body = document.querySelector("[data-slot='ai-panel']");
      expect(body).not.toBeNull();
      expect(isPanelBodyHidden(body!)).toBe(true);
    });

    // The collapsed state was persisted per board.
    const stored = JSON.parse(
      localStorage.getItem(aiPanelStateStorageKey(BOARD))!,
    );
    expect(stored.open).toBe(false);

    // The collapsed rail now exposes a single AI-star button as its toggle.
    // There must be no leftover `panel-right-open` icon from the old design
    // and the lone button must carry the sparkles icon.
    const rail = screen.getByTestId("ai-panel-container");
    const railButtons = within(rail).getAllByRole("button");
    expect(
      railButtons,
      "the collapsed rail must contain exactly one button (the star toggle)",
    ).toHaveLength(1);
    const railToggle = within(rail).getByRole("button", {
      name: /expand ai panel/i,
    });
    expect(
      railToggle.querySelector(".lucide-sparkles"),
      "the rail expand button must use the sparkles icon",
    ).not.toBeNull();
    expect(
      rail.querySelector(".lucide-panel-right-open"),
      "the rail must not render the legacy panel-right-open icon",
    ).toBeNull();

    // Remount — the container reads the persisted state back and stays collapsed.
    unmount();
    await renderContainer();
    await screen.findByTestId("ai-panel-container");
    const remountedBody = document.querySelector("[data-slot='ai-panel']");
    expect(remountedBody).not.toBeNull();
    expect(isPanelBodyHidden(remountedBody!)).toBe(true);

    // Expanding again persists `open: true` and reveals the body.
    const expand = screen.getByRole("button", { name: /expand ai panel/i });
    await act(async () => {
      fireEvent.click(expand);
    });
    await waitFor(() => {
      const body = document.querySelector("[data-slot='ai-panel']");
      expect(body).not.toBeNull();
      expect(isPanelBodyHidden(body!)).toBe(false);
    });
    const reopened = JSON.parse(
      localStorage.getItem(aiPanelStateStorageKey(BOARD))!,
    );
    expect(reopened.open).toBe(true);
  });

  it("conversation survives a toggle: collapsing then re-expanding preserves the messages", async () => {
    // Preseed a chosen model so the container mounts `AiPanelConversation` and
    // its `useConversation` store — without a model the panel renders the
    // no-model state instead.
    localStorage.setItem(
      aiPanelStateStorageKey(BOARD),
      JSON.stringify({ open: true, modelId: "claude-code" }),
    );

    const REPLY = "persistent agent reply";
    const harness = mockHarness({
      updates: [
        {
          sessionUpdate: "agent_message_chunk",
          content: { type: "text", text: REPLY },
        },
      ],
    });

    await renderContainer({ createConnect: harness.createConnect });
    await screen.findByTestId("ai-panel-container");

    // Drive one turn through the panel so the conversation store has a
    // message; both the user's prompt and the streamed reply must render.
    const textarea = await screen.findByRole("textbox", {
      name: /message the ai agent/i,
    });
    await act(async () => {
      await userEvent.type(textarea, "hello panel");
    });
    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: /submit/i }));
    });
    await waitFor(() => {
      expect(document.body.textContent).toContain(REPLY);
      expect(document.body.textContent).toContain("hello panel");
    });

    // The hosted ACP session that streamed the reply.
    const sessionBeforeToggle = harness.sessions()[0];
    expect(sessionBeforeToggle).toBeDefined();
    const connectCountBeforeToggle = harness.connectedModels().length;

    // Collapse via the window-layer `ai.toggle` command.
    await act(async () => {
      triggerAiToggle();
    });

    // The panel body is hidden but still mounted, and the reply text is still
    // in the DOM — the conversation survived the collapse.
    await waitFor(() => {
      const body = document.querySelector("[data-slot='ai-panel']");
      expect(body).not.toBeNull();
      expect(isPanelBodyHidden(body!)).toBe(true);
    });
    expect(document.body.textContent).toContain(REPLY);
    expect(document.body.textContent).toContain("hello panel");

    // Re-expand via the same command.
    await act(async () => {
      triggerAiToggle();
    });
    await waitFor(() => {
      const body = document.querySelector("[data-slot='ai-panel']");
      expect(body).not.toBeNull();
      expect(isPanelBodyHidden(body!)).toBe(false);
    });

    // After re-expanding both the user prompt and the assistant reply are
    // still rendered — no fresh conversation, no reset store.
    expect(document.body.textContent).toContain(REPLY);
    expect(document.body.textContent).toContain("hello panel");

    // The ACP session itself was preserved — `createConnect` was not invoked
    // again, and no new session was started on the re-expand.
    expect(harness.connectedModels().length).toBe(connectCountBeforeToggle);
    expect(harness.sessions()).toHaveLength(1);
    expect(harness.sessions()[0]).toBe(sessionBeforeToggle);
  });

  it("switching the active board starts a fresh ACP session against the new board's cwd", async () => {
    // Regression: per the AI panel task, switching to a different kanban board
    // must tear down the prior ACP client + session and issue a fresh
    // `newSession` whose `cwd` is the new board directory — the agent (and
    // the per-board MCP server) must see the new cwd, not the prior board's.
    //
    // The production path keys `<AiPanelConversation>` on a composite of
    // `${boardDir}::${modelId}`, so a board change remounts the conversation
    // and its `useConversation` refs are freshly initialized — `connect` is
    // re-invoked against the new factory built by `useProductionConnect`. The
    // harness here mirrors that contract: each board gets its own
    // `mockHarness(_, boardDir)`, and the test rerenders with the new board
    // path AND the new harness in one pass.
    const BOARD_A = "/tmp/board-a";
    const BOARD_B = "/tmp/board-b";
    const REPLY = "reply for board a";

    // Preseed a chosen model so the panel mounts `AiPanelConversation`
    // immediately — without a model the panel renders the no-model state and
    // never connects.
    localStorage.setItem(
      aiPanelStateStorageKey(BOARD_A),
      JSON.stringify({ open: true, modelId: "claude-code" }),
    );
    localStorage.setItem(
      aiPanelStateStorageKey(BOARD_B),
      JSON.stringify({ open: true, modelId: "claude-code" }),
    );

    const harnessA = mockHarness(
      {
        updates: [
          {
            sessionUpdate: "agent_message_chunk",
            content: { type: "text", text: REPLY },
          },
        ],
      },
      BOARD_A,
    );

    const { rerender } = await renderInAct(
      <BoardScopedContainer
        boardPath={BOARD_A}
        createConnect={harnessA.createConnect}
      />,
    );
    await screen.findByTestId("ai-panel-container");

    // Send a prompt against board A so `ensureSession` fires and the harness
    // captures a session tagged with board A's cwd.
    const textareaA = await screen.findByRole("textbox", {
      name: /message the ai agent/i,
    });
    await act(async () => {
      await userEvent.type(textareaA, "first prompt");
    });
    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: /submit/i }));
    });
    await waitFor(() => {
      expect(harnessA.sessions()).toHaveLength(1);
    });
    expect(harnessA.sessions()[0].cwd).toBe(BOARD_A);

    // Switch to board B — production hands the container a fresh
    // `createConnect` (memoized on the new boardDir) at the same time the
    // `ActiveBoardPathProvider` value flips, so the harness pattern matches:
    // a new `mockHarness` bound to board B accompanies the rerender.
    const harnessB = mockHarness({}, BOARD_B);
    await act(async () => {
      rerender(
        <BoardScopedContainer
          boardPath={BOARD_B}
          createConnect={harnessB.createConnect}
        />,
      );
    });

    // The board-A session must NOT be reused for board B — sending a prompt
    // against the new board triggers a brand-new connect + newSession on the
    // new harness.
    const textareaB = await screen.findByRole("textbox", {
      name: /message the ai agent/i,
    });
    await act(async () => {
      await userEvent.type(textareaB, "second prompt");
    });
    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: /submit/i }));
    });

    await waitFor(() => {
      expect(harnessB.sessions()).toHaveLength(1);
    });
    // The board-B session was started against board B's cwd — the production
    // analogue is `cwd: BOARD_B` in `newSession`.
    expect(harnessB.sessions()[0].cwd).toBe(BOARD_B);
    // And no extra session was started on the old (board-A) harness.
    expect(harnessA.sessions()).toHaveLength(1);
  });

  it("re-selecting the same board does not tear the session down", async () => {
    // The flip side of the board-switch regression: a no-op switch (the
    // user picks the same board again, or the active-board value re-emits)
    // must NOT remount `AiPanelConversation`. The composite key
    // `${boardDir}::${modelId}` is stable when boardDir and modelId are
    // both stable, so the existing client + session survive.
    localStorage.setItem(
      aiPanelStateStorageKey(BOARD),
      JSON.stringify({ open: true, modelId: "claude-code" }),
    );

    const harness = mockHarness(
      {
        updates: [
          {
            sessionUpdate: "agent_message_chunk",
            content: { type: "text", text: "ack" },
          },
        ],
      },
      BOARD,
    );

    const { rerender } = await renderInAct(
      <BoardScopedContainer
        boardPath={BOARD}
        createConnect={harness.createConnect}
      />,
    );
    await screen.findByTestId("ai-panel-container");

    const textarea = await screen.findByRole("textbox", {
      name: /message the ai agent/i,
    });
    await act(async () => {
      await userEvent.type(textarea, "hello");
    });
    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: /submit/i }));
    });
    await waitFor(() => {
      expect(harness.sessions()).toHaveLength(1);
    });
    const sessionBefore = harness.sessions()[0];

    // Rerender with the SAME board path and the SAME `createConnect`. The
    // composite key is unchanged so `AiPanelConversation` is NOT remounted
    // and the cached client + session survive.
    await act(async () => {
      rerender(
        <BoardScopedContainer
          boardPath={BOARD}
          createConnect={harness.createConnect}
        />,
      );
    });

    // A second prompt reuses the existing session — no new connect, no new
    // `newSession`.
    await act(async () => {
      await userEvent.type(textarea, "again");
    });
    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: /submit/i }));
    });

    await waitFor(() => {
      expect(sessionBefore.prompts.length).toBe(2);
    });
    expect(harness.sessions()).toHaveLength(1);
    expect(harness.sessions()[0]).toBe(sessionBefore);
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
    const openBody = document.querySelector("[data-slot='ai-panel']");
    expect(openBody).not.toBeNull();
    expect(isPanelBodyHidden(openBody!)).toBe(false);

    // The container registered its `toggle` handler into the AI command
    // registry; firing it (as the window-layer `ai.toggle` command does)
    // collapses the panel — the body stays mounted but is hidden so the
    // conversation survives the toggle.
    await act(async () => {
      triggerAiToggle();
    });
    await waitFor(() => {
      const body = document.querySelector("[data-slot='ai-panel']");
      expect(body).not.toBeNull();
      expect(isPanelBodyHidden(body!)).toBe(true);
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
      const body = document.querySelector("[data-slot='ai-panel']");
      expect(body).not.toBeNull();
      expect(isPanelBodyHidden(body!)).toBe(false);
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
    const collapsedBody = document.querySelector("[data-slot='ai-panel']");
    expect(collapsedBody).not.toBeNull();
    expect(isPanelBodyHidden(collapsedBody!)).toBe(true);

    // Firing the registered `ai.focus` handler expands the panel and moves
    // focus into the prompt editor — the AI composer's CM6 content DOM,
    // located by its `role="textbox"` + accessible label.
    await act(async () => {
      triggerAiFocus();
    });
    await waitFor(() => {
      const body = document.querySelector("[data-slot='ai-panel']");
      expect(body).not.toBeNull();
      expect(isPanelBodyHidden(body!)).toBe(false);
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
    // With two available models in the list, the auto-select default lands on
    // the first (Claude Code). This test then drives a user-initiated pick of
    // the *second* model through the composer's footer select, asserting that
    // the user pick overrides the auto-selected default and is persisted —
    // and that a fresh mount reapplies the user's choice, not the default.
    const TWO_MODELS: AiModel[] = [
      {
        id: "claude-code",
        label: "Claude Code",
        kind: "claude-code",
        available: true,
        hint: "Claude Code CLI: /usr/local/bin/claude",
      },
      {
        id: "qwen-coder",
        label: "Qwen Coder",
        kind: "local-llama",
        available: true,
        hint: null,
      },
    ];
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "ai_list_models") return TWO_MODELS;
      return undefined;
    });

    await renderContainer();
    await screen.findByTestId("ai-panel-container");

    // The auto-select default landed on Claude Code; the trigger reads its
    // label now (not the placeholder).
    const selector = await waitFor(() => {
      const btn = screen.getByRole("combobox", { name: /claude code/i });
      expect(btn).not.toBeDisabled();
      return btn;
    });

    // User picks the second model — the explicit choice overrides the
    // auto-selected default.
    await act(async () => {
      await userEvent.click(selector);
    });
    const listbox = await screen.findByRole("listbox");
    await act(async () => {
      await userEvent.click(
        within(listbox).getByRole("option", { name: /qwen coder/i }),
      );
    });

    // The Container persisted the user-picked per-board model id.
    await waitFor(() => {
      const stored = JSON.parse(
        localStorage.getItem(aiPanelStateStorageKey(BOARD))!,
      );
      expect(stored.modelId).toBe("qwen-coder");
    });

    // A fresh mount reapplies the user's pick — the selector reads its label.
    await renderContainer();
    await waitFor(() => {
      const triggers = screen.getAllByRole("combobox", {
        name: /qwen coder/i,
      });
      expect(triggers.length).toBeGreaterThan(0);
    });
  });

  it("auto-selects the first available model on a fresh mount with no persisted modelId", async () => {
    // The panel must never land in the dead-end `NoModelState` when
    // `ai_list_models` already returned a usable model. On a board with no
    // persisted `ai-panel-state:<path>` entry, the Container picks the first
    // `available: true` model — Claude Code in this fixture — and persists it
    // through the same `saveAiPanelState` path a user click would take. The
    // panel transitions out of the no-model state within one render, and a
    // remount reads the model id back from `localStorage`.
    const AVAILABLE_MODELS: AiModel[] = [
      {
        id: "claude-code",
        label: "Claude Code",
        kind: "claude-code",
        available: true,
        hint: "Claude Code CLI: /usr/local/bin/claude",
      },
      {
        id: "qwen-coder",
        label: "Qwen Coder",
        kind: "local-llama",
        available: true,
        hint: null,
      },
    ];
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "ai_list_models") return AVAILABLE_MODELS;
      return undefined;
    });

    // No persisted state for this board.
    expect(localStorage.getItem(aiPanelStateStorageKey(BOARD))).toBeNull();

    await renderContainer();
    await screen.findByTestId("ai-panel-container");

    // After `ai_list_models` resolves, the per-board modelId becomes
    // `claude-code`, persisted to the board's `localStorage` snapshot.
    await waitFor(() => {
      const raw = localStorage.getItem(aiPanelStateStorageKey(BOARD));
      expect(raw).not.toBeNull();
      const stored = JSON.parse(raw!);
      expect(stored.modelId).toBe("claude-code");
    });

    // The panel left the no-model state — its composer trigger now reads the
    // selected model label, the same surface as the persisted-model test
    // above. The `NoModelState` "No AI models are configured." copy must not
    // be rendered.
    await waitFor(() => {
      const triggers = screen.getAllByRole("combobox", {
        name: /claude code/i,
      });
      expect(triggers.length).toBeGreaterThan(0);
    });
    expect(document.body.textContent).not.toContain(
      "No AI models are configured.",
    );
  });

  it("auto-selects the lone model even when it is unavailable", async () => {
    // Regression: once the panel is filtered down to only `claude-code`, the
    // sole entry is `available: false` whenever the running app can't find the
    // `claude` CLI on its PATH (the bundled macOS GUI does not inherit the
    // shell PATH). The Container must still select it — the only option — so
    // the panel surfaces the model and its install hint instead of stranding
    // the user in `NoModelState` with no idea why nothing is selected.
    const UNAVAILABLE_MODELS: AiModel[] = [
      {
        id: "claude-code",
        label: "Claude Code",
        kind: "claude-code",
        available: false,
        hint: "Claude Code CLI not found — install it and ensure `claude` is on your PATH.",
      },
    ];
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "ai_list_models") return UNAVAILABLE_MODELS;
      return undefined;
    });

    await renderContainer();
    await screen.findByTestId("ai-panel-container");

    // The lone (unavailable) model is auto-selected and persisted, exactly as
    // an available default would be.
    await waitFor(() => {
      const raw = localStorage.getItem(aiPanelStateStorageKey(BOARD));
      expect(raw).not.toBeNull();
      const stored = JSON.parse(raw!);
      expect(stored.modelId).toBe("claude-code");
    });

    // The panel left the `NoModelState` branch — its "Choose a model" heading
    // is gone because `AiPanelConversation` (mounted only when a model is
    // selected) renders instead.
    await waitFor(() => {
      expect(document.body.textContent).not.toContain("Choose a model");
    });
  });

  it("re-selects a default when the persisted model is no longer offered", async () => {
    // A previously-picked model can drop out of `ai_list_models` — e.g. a
    // llama model that lost its `kanban` tag. A stale persisted id must not
    // strand the panel: when the persisted id is absent from the current list,
    // the Container falls back to a default just as if nothing were persisted.
    localStorage.setItem(
      aiPanelStateStorageKey(BOARD),
      JSON.stringify({ modelId: "qwen-coder" }),
    );

    const MODELS: AiModel[] = [
      {
        id: "claude-code",
        label: "Claude Code",
        kind: "claude-code",
        available: false,
        hint: "Claude Code CLI not found — install it and ensure `claude` is on your PATH.",
      },
    ];
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "ai_list_models") return MODELS;
      return undefined;
    });

    await renderContainer();
    await screen.findByTestId("ai-panel-container");

    // The stale `qwen-coder` id is replaced by the only offered model.
    await waitFor(() => {
      const stored = JSON.parse(
        localStorage.getItem(aiPanelStateStorageKey(BOARD))!,
      );
      expect(stored.modelId).toBe("claude-code");
    });
  });

  it("does not overwrite a persisted modelId, even when the persisted model is unavailable", async () => {
    // A persisted `modelId` is an explicit prior user pick — the auto-select
    // effect must be a no-op against it. Even if the persisted model is
    // currently `available: false`, the user's choice wins; the Container
    // must not silently swap them onto the first available model.
    localStorage.setItem(
      aiPanelStateStorageKey(BOARD),
      JSON.stringify({ modelId: "qwen-coder" }),
    );

    const MIXED_MODELS: AiModel[] = [
      {
        id: "claude-code",
        label: "Claude Code",
        kind: "claude-code",
        available: true,
        hint: "Claude Code CLI: /usr/local/bin/claude",
      },
      {
        id: "qwen-coder",
        label: "Qwen Coder",
        kind: "local-llama",
        available: false,
        hint: null,
      },
    ];
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "ai_list_models") return MIXED_MODELS;
      return undefined;
    });

    await renderContainer();
    await screen.findByTestId("ai-panel-container");

    // Settle the model-list effect.
    await waitFor(() => {
      const triggers = screen.queryAllByRole("combobox");
      expect(triggers.length).toBeGreaterThan(0);
    });
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    // The persisted `qwen-coder` choice survives — the composer trigger shows
    // its label, and `localStorage` still holds the original id.
    await waitFor(() => {
      const triggers = screen.getAllByRole("combobox", {
        name: /qwen coder/i,
      });
      expect(triggers.length).toBeGreaterThan(0);
    });
    const stored = JSON.parse(
      localStorage.getItem(aiPanelStateStorageKey(BOARD))!,
    );
    expect(stored.modelId).toBe("qwen-coder");
  });
});
