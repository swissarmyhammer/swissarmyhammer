/**
 * Component tests for {@link AiPanel}.
 *
 * `AiPanel` is a View (see `ARCHITECTURE.md` Container/View separation): it
 * renders the conversation, the model selector, the permission prompt, and the
 * composer, and it never touches the Tauri backend directly. Every backend
 * seam — model enumeration, agent start — is an injected prop, so these tests
 * drive the panel against a hand-written **mock ACP client** with no transport
 * and no `invoke`.
 *
 * Browser project (`*.test.tsx`) — the AI Elements components mount in real
 * Chromium. The genuine ACP protocol plumbing is covered by
 * `acp-client.node.test.ts` / `conversation.test.tsx`; this file covers the
 * panel's own wiring on top of {@link useConversation}.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "vitest/browser";
import type {
  ContentBlock,
  PromptResponse,
  RequestPermissionRequest,
  RequestPermissionResponse,
  SessionNotification,
} from "@agentclientprotocol/sdk";
import { renderInAct } from "@/test/act-render";
import type {
  AcpSession,
  KanbanAcpClient,
  RequestPermissionHandler,
  SessionUpdateHandler,
} from "@/ai/acp-client";
import type { ConversationConnect } from "@/ai/conversation";
import {
  aiStreaming,
  resetAiCommandsForTest,
  triggerAiCancel,
  triggerAiNewChat,
} from "@/ai/commands";
import { AiPanel, type AiModel, type AiPanelConnectFactory } from "./ai-panel";

/** A plain ACP text content block. */
function textBlock(text: string): ContentBlock {
  return { type: "text", text };
}

/** The script a {@link FakeSession} replays when `prompt` is called. */
interface SessionScript {
  /** `session/update` notifications streamed before `prompt` resolves. */
  updates?: SessionNotification["update"][];
  /** The stop reason `prompt` resolves with (default `end_turn`). */
  stopReason?: PromptResponse["stopReason"];
}

/**
 * A controllable fake ACP session.
 *
 * `prompt` streams the scripted notifications to the captured
 * `onSessionUpdate` handler, then resolves with the scripted stop reason —
 * the same observable shape as a real turn, with no WebSocket.
 */
class FakeSession implements AcpSession {
  readonly sessionId = "fake-session";
  /** Every prompt the panel sent, in order. */
  readonly prompts: ContentBlock[][] = [];
  /** Whether `cancel` was called. */
  cancelled = false;

  constructor(
    private readonly onUpdate: SessionUpdateHandler,
    private readonly script: SessionScript,
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

/**
 * A fake ACP session whose `prompt` never resolves on its own.
 *
 * Models a long-running turn: the panel stays in the streaming state until
 * `cancel` is called, which both records the cancellation and resolves the
 * pending `prompt` with a `cancelled` stop reason.
 */
class HangingSession implements AcpSession {
  readonly sessionId = "hanging-session";
  /** Whether `cancel` was called. */
  cancelled = false;
  /** Resolver for the pending `prompt` promise, set when `prompt` is called. */
  private resolvePrompt: ((response: PromptResponse) => void) | undefined;

  prompt(): Promise<PromptResponse> {
    return new Promise<PromptResponse>((resolve) => {
      this.resolvePrompt = resolve;
    });
  }

  async cancel(): Promise<void> {
    this.cancelled = true;
    this.resolvePrompt?.({ stopReason: "cancelled" });
  }

  async setMode(): Promise<void> {
    // Unused by these tests.
  }
}

/** A connected mock ACP client plus the seams a test inspects after the fact. */
interface MockHarness {
  /** The {@link AiPanelConnectFactory} to pass to {@link AiPanel}. */
  createConnect: AiPanelConnectFactory;
  /** Every model id `createConnect` was invoked with, in order. */
  connectedModels: () => string[];
  /** Every session the fake client started, in order. */
  sessions: () => FakeSession[];
  /** The `onRequestPermission` handler the latest client captured. */
  permission: () => RequestPermissionHandler;
}

/**
 * Build a {@link AiPanelConnectFactory} backed by {@link FakeSession}s.
 *
 * The returned factory records which model ids the panel connected, and
 * exposes the constructed sessions and the captured permission handler so a
 * test can drive the agent side of the conversation.
 */
function mockHarness(script: SessionScript = {}): MockHarness {
  const connectedModels: string[] = [];
  const sessions: FakeSession[] = [];
  let capturedPermission: RequestPermissionHandler | undefined;

  const createConnect: AiPanelConnectFactory = (modelId) => {
    connectedModels.push(modelId);
    const connect: ConversationConnect = async (handlers) => {
      capturedPermission = handlers.onRequestPermission;
      const client: KanbanAcpClient = {
        protocolVersion: 1,
        initializeResponse: { protocolVersion: 1, agentCapabilities: {} },
        async startSession(): Promise<AcpSession> {
          const session = new FakeSession(handlers.onSessionUpdate, script);
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
    permission: () => {
      if (!capturedPermission) {
        throw new Error("createConnect was never invoked");
      }
      return capturedPermission;
    },
  };
}

/** The Claude Code + a disabled local model fixture used across selector tests. */
const MODELS: AiModel[] = [
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
    hint: "Model weights unavailable on this machine.",
  },
];

describe("AiPanel: conversation rendering", () => {
  it("renders streamed assistant text, a reasoning block, and a tool card", async () => {
    const harness = mockHarness({
      updates: [
        {
          sessionUpdate: "agent_thought_chunk",
          content: textBlock("Considering the request."),
        },
        {
          sessionUpdate: "agent_message_chunk",
          content: textBlock("Here is the **answer**."),
        },
        {
          sessionUpdate: "tool_call",
          toolCallId: "call-1",
          title: "kanban__list_tasks",
          kind: "other",
          status: "completed",
          rawInput: { column: "doing" },
          rawOutput: { tasks: 2 },
        },
        {
          sessionUpdate: "plan",
          entries: [
            {
              content: "Read the board",
              priority: "high",
              status: "completed",
            },
            {
              content: "Summarize tasks",
              priority: "medium",
              status: "pending",
            },
          ],
        },
      ],
    });

    await renderInAct(
      <AiPanel
        boardDir="/tmp/board"
        models={MODELS}
        modelId="claude-code"
        onSelectModel={() => {}}
        createConnect={harness.createConnect}
      />,
    );

    // Send a prompt — the fake session replays the scripted updates.
    const textarea = screen.getByRole("textbox");
    await act(async () => {
      await userEvent.type(textarea, "what is in progress?");
    });
    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: /submit/i }));
    });

    // The user's prompt and the streamed assistant reply both render.
    await waitFor(() => {
      expect(document.body.textContent).toContain("what is in progress?");
    });
    expect(document.body.textContent).toContain("Here is the");
    expect(document.body.textContent).toContain("answer");
    // The reasoning (thought) block rendered.
    expect(document.body.textContent).toContain("Considering the request.");
    // The tool-call card rendered with its tool name and completed status.
    expect(document.body.textContent).toContain("kanban__list_tasks");
    expect(document.body.textContent).toContain("Completed");
    // The agent's plan rendered its entries.
    expect(document.body.textContent).toContain("Summarize tasks");

    expect(harness.connectedModels()).toEqual(["claude-code"]);
    expect(harness.sessions()[0].prompts).toEqual([
      [textBlock("what is in progress?")],
    ]);
  });

  it("stop button cancels the in-flight turn", async () => {
    // A turn that never resolves on its own — `prompt` hangs so the panel
    // stays in the streaming state and the stop button is the only way out.
    const sessions: HangingSession[] = [];
    const createConnect: AiPanelConnectFactory = () => async () => ({
      protocolVersion: 1,
      initializeResponse: { protocolVersion: 1, agentCapabilities: {} },
      async startSession(): Promise<AcpSession> {
        const session = new HangingSession();
        sessions.push(session);
        return session;
      },
    });

    await renderInAct(
      <AiPanel
        boardDir="/tmp/board"
        models={MODELS}
        modelId="claude-code"
        onSelectModel={() => {}}
        createConnect={createConnect}
      />,
    );

    const textarea = screen.getByRole("textbox");
    await act(async () => {
      await userEvent.type(textarea, "long task");
    });
    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: /submit/i }));
    });

    // The submit button flips to a stop affordance while streaming.
    const stop = await screen.findByRole("button", { name: /stop/i });
    await act(async () => {
      await userEvent.click(stop);
    });

    await waitFor(() => {
      expect(sessions[0].cancelled).toBe(true);
    });
  });

  it("'New conversation' clears the message log", async () => {
    const harness = mockHarness({
      updates: [
        {
          sessionUpdate: "agent_message_chunk",
          content: textBlock("first reply"),
        },
      ],
    });

    await renderInAct(
      <AiPanel
        boardDir="/tmp/board"
        models={MODELS}
        modelId="claude-code"
        onSelectModel={() => {}}
        createConnect={harness.createConnect}
      />,
    );

    const textarea = screen.getByRole("textbox");
    await act(async () => {
      await userEvent.type(textarea, "hello there");
    });
    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: /submit/i }));
    });
    await waitFor(() => {
      expect(document.body.textContent).toContain("first reply");
    });

    await act(async () => {
      await userEvent.click(
        screen.getByRole("button", { name: /new conversation/i }),
      );
    });

    await waitFor(() => {
      expect(document.body.textContent).not.toContain("first reply");
    });
    expect(document.body.textContent).not.toContain("hello there");
  });
});

describe("AiPanel: model selector", () => {
  it("lists models, disables unavailable entries, and shows their hint", async () => {
    const harness = mockHarness();

    await renderInAct(
      <AiPanel
        boardDir="/tmp/board"
        models={MODELS}
        modelId="claude-code"
        onSelectModel={() => {}}
        createConnect={harness.createConnect}
      />,
    );

    // Open the model selector in the header.
    await act(async () => {
      await userEvent.click(
        screen.getByRole("button", { name: /claude code/i }),
      );
    });

    const menu = await screen.findByRole("menu");
    const items = within(menu).getAllByRole("menuitem");
    expect(items).toHaveLength(2);

    const claude = items[0];
    const qwen = items[1];
    expect(claude.textContent).toContain("Claude Code");
    // The unavailable local model is disabled and surfaces its hint.
    expect(qwen.getAttribute("aria-disabled")).toBe("true");
    expect(qwen.textContent).toContain("Model weights unavailable");
  });

  it("selecting a model reports the choice and starts a fresh ACP session", async () => {
    const harness = mockHarness({
      updates: [
        { sessionUpdate: "agent_message_chunk", content: textBlock("ok") },
      ],
    });
    const onSelectModel = vi.fn();

    // Two available models so a second one can be picked.
    const bothAvailable: AiModel[] = [
      { ...MODELS[0] },
      { ...MODELS[1], available: true, hint: "Local model." },
    ];

    const { rerender } = await renderInAct(
      <AiPanel
        boardDir="/tmp/board"
        models={bothAvailable}
        modelId="claude-code"
        onSelectModel={onSelectModel}
        createConnect={harness.createConnect}
      />,
    );

    await act(async () => {
      await userEvent.click(
        screen.getByRole("button", { name: /claude code/i }),
      );
    });
    const menu = await screen.findByRole("menu");
    await act(async () => {
      await userEvent.click(
        within(menu).getByRole("menuitem", { name: /qwen coder/i }),
      );
    });

    // The panel reports the choice so the container can persist it per board.
    expect(onSelectModel).toHaveBeenCalledWith("qwen-coder");

    // The container persists and feeds the new id back as a prop.
    await act(async () => {
      rerender(
        <AiPanel
          boardDir="/tmp/board"
          models={bothAvailable}
          modelId="qwen-coder"
          onSelectModel={onSelectModel}
          createConnect={harness.createConnect}
        />,
      );
    });

    // A prompt after the switch connects through the newly selected model.
    const textarea = screen.getByRole("textbox");
    await act(async () => {
      await userEvent.type(textarea, "ping");
    });
    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: /submit/i }));
    });

    await waitFor(() => {
      expect(harness.connectedModels()).toContain("qwen-coder");
    });
  });

  it("the composer is disabled until a model is selected", async () => {
    const harness = mockHarness();

    await renderInAct(
      <AiPanel
        boardDir="/tmp/board"
        models={MODELS}
        modelId={null}
        onSelectModel={() => {}}
        createConnect={harness.createConnect}
      />,
    );

    expect(screen.getByRole("textbox")).toBeDisabled();
  });
});

describe("AiPanel: permission prompt", () => {
  it("renders an inline prompt and a click resolves the request", async () => {
    const harness = mockHarness({
      updates: [
        { sessionUpdate: "agent_message_chunk", content: textBlock("working") },
      ],
    });

    await renderInAct(
      <AiPanel
        boardDir="/tmp/board"
        models={MODELS}
        modelId="claude-code"
        onSelectModel={() => {}}
        createConnect={harness.createConnect}
      />,
    );

    // Prime the connection so the permission handler is captured.
    const textarea = screen.getByRole("textbox");
    await act(async () => {
      await userEvent.type(textarea, "edit the config");
    });
    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: /submit/i }));
    });

    const request: RequestPermissionRequest = {
      sessionId: "fake-session",
      toolCall: {
        toolCallId: "call-1",
        title: "Edit kanban board",
        kind: "edit",
        status: "pending",
      },
      options: [
        { kind: "allow_once", name: "Allow once", optionId: "allow" },
        { kind: "allow_always", name: "Allow for session", optionId: "always" },
        { kind: "reject_once", name: "Deny", optionId: "deny" },
      ],
    };

    let decision: Promise<RequestPermissionResponse> | undefined;
    await act(async () => {
      decision = harness.permission()(request);
    });

    // The inline approval UI renders the tool title and every option.
    await waitFor(() => {
      expect(document.body.textContent).toContain("Edit kanban board");
    });
    expect(
      screen.getByRole("button", { name: /allow for session/i }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /deny/i })).toBeInTheDocument();

    // Clicking an option resolves the agent's request with that option id.
    await act(async () => {
      await userEvent.click(
        screen.getByRole("button", { name: /allow for session/i }),
      );
    });

    await expect(decision).resolves.toEqual({
      outcome: { outcome: "selected", optionId: "always" },
    });
    // The prompt is dismissed once answered.
    await waitFor(() => {
      expect(
        screen.queryByRole("button", { name: /allow for session/i }),
      ).toBeNull();
    });
  });
});

/**
 * The AI panel command scope — `AiPanelConversation` registers `ai.newChat`
 * and `ai.cancel` handlers into the `ai/commands.ts` module registry and
 * mirrors the ACP turn status into the registry's streaming flag. These tests
 * drive the registered handlers (the same way the window-layer `ai.*` commands
 * do) and the streaming flag, with no `AppShell` in the tree.
 */
describe("AiPanel: ai.* command integration", () => {
  beforeEach(() => {
    resetAiCommandsForTest();
  });

  it("the ai.newChat handler clears the conversation/session", async () => {
    const harness = mockHarness({
      updates: [
        { sessionUpdate: "agent_message_chunk", content: textBlock("a reply") },
      ],
    });

    await renderInAct(
      <AiPanel
        boardDir="/tmp/board"
        models={MODELS}
        modelId="claude-code"
        onSelectModel={() => {}}
        createConnect={harness.createConnect}
      />,
    );

    // Run a turn so there is a session and a message log to clear.
    const textarea = screen.getByRole("textbox");
    await act(async () => {
      await userEvent.type(textarea, "hello there");
    });
    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: /submit/i }));
    });
    await waitFor(() => {
      expect(document.body.textContent).toContain("a reply");
    });

    // Fire the registered `ai.newChat` handler — the conversation resets.
    await act(async () => {
      triggerAiNewChat();
    });
    await waitFor(() => {
      expect(document.body.textContent).not.toContain("a reply");
    });
    expect(document.body.textContent).not.toContain("hello there");

    // The next prompt opens a brand-new stateless session.
    await act(async () => {
      await userEvent.type(screen.getByRole("textbox"), "second");
    });
    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: /submit/i }));
    });
    await waitFor(() => {
      expect(harness.sessions().length).toBe(2);
    });
  });

  it("ai.cancel availability tracks streaming, and the handler cancels the turn", async () => {
    // A hanging turn keeps the conversation streaming until cancelled.
    const sessions: HangingSession[] = [];
    const createConnect: AiPanelConnectFactory = () => async () => ({
      protocolVersion: 1,
      initializeResponse: { protocolVersion: 1, agentCapabilities: {} },
      async startSession(): Promise<AcpSession> {
        const session = new HangingSession();
        sessions.push(session);
        return session;
      },
    });

    await renderInAct(
      <AiPanel
        boardDir="/tmp/board"
        models={MODELS}
        modelId="claude-code"
        onSelectModel={() => {}}
        createConnect={createConnect}
      />,
    );

    // Idle before any turn — the streaming flag (which gates `ai.cancel`'s
    // availability) is false.
    expect(aiStreaming()).toBe(false);

    await act(async () => {
      await userEvent.type(screen.getByRole("textbox"), "long task");
    });
    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: /submit/i }));
    });

    // The turn is in flight — streaming is reported true, so `ai.cancel`
    // becomes available.
    await waitFor(() => {
      expect(aiStreaming()).toBe(true);
    });

    // Fire the registered `ai.cancel` handler — the in-flight turn is
    // cancelled and the conversation leaves the streaming state.
    await act(async () => {
      triggerAiCancel();
    });
    await waitFor(() => {
      expect(sessions[0].cancelled).toBe(true);
    });
    await waitFor(() => {
      expect(aiStreaming()).toBe(false);
    });
  });
});
