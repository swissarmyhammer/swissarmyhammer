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
  CreateElicitationRequest,
  CreateElicitationResponse,
  PromptResponse,
  RequestPermissionRequest,
  RequestPermissionResponse,
  SessionNotification,
} from "@agentclientprotocol/sdk";
import { renderInAct } from "@/test/act-render";
import type {
  AcpSession,
  ElicitationHandler,
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
  /**
   * Ambient `session/update`s streamed during `startSession` — before any
   * prompt — mirroring how the real claude backend forwards the CLI's slash
   * commands as an `available_commands_update` at session start. The warm-up
   * path relies on these arriving without a prompt being sent.
   */
  initUpdates?: SessionNotification["update"][];
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
  /** The `onElicitation` handler the latest client captured. */
  elicitation: () => ElicitationHandler;
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
  let capturedElicitation: ElicitationHandler | undefined;

  const createConnect: AiPanelConnectFactory = (modelId) => {
    connectedModels.push(modelId);
    const connect: ConversationConnect = async (handlers) => {
      capturedPermission = handlers.onRequestPermission;
      capturedElicitation = handlers.onElicitation;
      const client: KanbanAcpClient = {
        protocolVersion: 1,
        initializeResponse: { protocolVersion: 1, agentCapabilities: {} },
        async startSession(): Promise<AcpSession> {
          const session = new FakeSession(handlers.onSessionUpdate, script);
          sessions.push(session);
          // Replay init-time ambient updates (e.g. available_commands_update)
          // exactly as the real agent does during `session/new`, before any
          // prompt — this is what the warm-up path depends on.
          for (const update of script.initUpdates ?? []) {
            await handlers.onSessionUpdate({
              sessionId: session.sessionId,
              update,
            });
          }
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
    elicitation: () => {
      if (!capturedElicitation) {
        throw new Error("createConnect was never invoked");
      }
      return capturedElicitation;
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

/**
 * The width-determining Tailwind utilities, translated to real CSS.
 *
 * The browser test project does not load `@tailwindcss/vite` (the plugin runs
 * only at app build time), so utility classes like `w-fit` / `w-full` carry no
 * CSS during tests — every element falls back to `display: block; width: auto`
 * and fills its parent, which makes `w-fit` and `w-full` indistinguishable by
 * layout. This shim is the same pattern the spatial/layout tests use (e.g.
 * `app-layout.test.tsx`): it defines exactly the classes that drive the
 * message-width relationship — the `Message` wrapper, the role-conditional
 * `MessageContent` width, and the tool card — so the `getBoundingClientRect()`
 * assertions exercise real Chromium layout.
 *
 * The role-conditional variants are matched via `[class~="…"]` attribute
 * selectors (whole-token match) scoped under `.is-user` / `.is-assistant`,
 * mirroring how Tailwind compiles `group-[.is-*]:` against the `group`
 * ancestor — without reproducing Tailwind's escaped class-name selectors.
 */
const MESSAGE_WIDTH_SHIM = `
.flex { display: flex; }
.flex-col { flex-direction: column; }
.w-full { width: 100%; }
.w-fit { width: fit-content; }
.min-w-0 { min-width: 0; }
.max-w-full { max-width: 100%; }
.max-w-\\[95\\%\\] { max-width: 95%; }
.overflow-hidden { overflow: hidden; }
.rounded-md { border-radius: 0.375rem; }
.border { border-width: 1px; border-style: solid; }
.is-assistant [class~="group-[.is-assistant]:w-full"] { width: 100%; }
.is-user [class~="group-[.is-user]:w-fit"] { width: fit-content; }
`;

/**
 * Inject {@link MESSAGE_WIDTH_SHIM} into the document head and return a cleanup
 * that removes it. Each test installs the shim for the duration of its render
 * and tears it down in a `finally` so the global stylesheet does not leak into
 * sibling tests (which assert on unstyled layout).
 */
function installMessageWidthShim(): () => void {
  const style = document.createElement("style");
  style.setAttribute("data-test", "ai-panel-message-width-shim");
  style.textContent = MESSAGE_WIDTH_SHIM;
  document.head.appendChild(style);
  return () => {
    style.remove();
  };
}

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
        onCollapse={() => {}}
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

  it("renders an assistant tool-call card at the full assistant message width", async () => {
    // Layout regression guard: assistant block content (tool folds) must span
    // the assistant message region, not shrink to the tool card's intrinsic
    // content width. A short tool name + tiny input/output keeps the card's
    // natural width well under the conversation column, so a `w-fit` wrapper
    // (the pre-fix behavior) collapses the card to that narrow content and this
    // assertion fails; the role-conditional `group-[.is-assistant]:w-full`
    // wrapper makes it span the column and pass.
    //
    // The browser test project does not load `@tailwindcss/vite`, so utility
    // classes carry no CSS on their own — see `installMessageWidthShim`. The
    // shim translates exactly the width-determining utilities (including the
    // role-conditional `group-[.is-*]:w-*` variants) into real CSS so the
    // `w-fit` vs `w-full` distinction lays out for real in Chromium.
    const cleanup = installMessageWidthShim();
    try {
      const harness = mockHarness({
        updates: [
          {
            sessionUpdate: "tool_call",
            toolCallId: "call-1",
            title: "ls",
            kind: "other",
            status: "completed",
            rawInput: { p: "." },
            rawOutput: { n: 1 },
          },
        ],
      });

      // A wide, finite-height host so the conversation column is far wider than
      // the collapsed tool header's intrinsic width — leaving room for `w-fit`
      // to shrink the card and `w-full` to fill the column.
      const { container } = await renderInAct(
        <div style={{ width: 1000, height: 600 }}>
          <AiPanel
            boardDir="/tmp/board"
            models={MODELS}
            modelId="claude-code"
            onSelectModel={() => {}}
            onCollapse={() => {}}
            createConnect={harness.createConnect}
          />
        </div>,
      );

      const textarea = screen.getByRole("textbox");
      await act(async () => {
        await userEvent.type(textarea, "run ls");
      });
      await act(async () => {
        await userEvent.click(screen.getByRole("button", { name: /submit/i }));
      });

      // Wait for the assistant tool card to mount.
      await waitFor(() => {
        expect(
          container.querySelector(".is-assistant [data-slot='collapsible']"),
        ).not.toBeNull();
      });

      const assistantMessage = container.querySelector(
        ".is-assistant",
      ) as HTMLElement;
      const toolCard = container.querySelector(
        ".is-assistant [data-slot='collapsible']",
      ) as HTMLElement;

      const messageRect = assistantMessage.getBoundingClientRect();
      const toolRect = toolCard.getBoundingClientRect();

      // The tool's intrinsic content ("ls" header + status badge) is far
      // narrower than the column; this only holds if the assistant content
      // wrapper went full-width. The 2px slack absorbs sub-pixel/border
      // rounding between the two rects.
      expect(toolRect.width).toBeGreaterThanOrEqual(messageRect.width - 2);
    } finally {
      cleanup();
    }
  });

  it("renders a short user message as a fit-width bubble, narrower than its column", async () => {
    // Companion guard to the assistant full-width test: the role-conditional
    // width fix must NOT make user messages full-width. A short user bubble
    // keeps `w-fit`, so its content box hugs the text and stays strictly
    // narrower than the conversation column. Same Tailwind shim as above so the
    // `w-fit` user-bubble path lays out for real.
    const cleanup = installMessageWidthShim();
    try {
      const harness = mockHarness({
        updates: [
          { sessionUpdate: "agent_message_chunk", content: textBlock("ok") },
        ],
      });

      const { container } = await renderInAct(
        <div style={{ width: 1000, height: 600 }}>
          <AiPanel
            boardDir="/tmp/board"
            models={MODELS}
            modelId="claude-code"
            onSelectModel={() => {}}
            onCollapse={() => {}}
            createConnect={harness.createConnect}
          />
        </div>,
      );

      const textarea = screen.getByRole("textbox");
      await act(async () => {
        await userEvent.type(textarea, "hi");
      });
      await act(async () => {
        await userEvent.click(screen.getByRole("button", { name: /submit/i }));
      });

      // Wait for the user message to render.
      await waitFor(() => {
        expect(container.querySelector(".is-user")).not.toBeNull();
      });

      const userMessage = container.querySelector(".is-user") as HTMLElement;
      // The user bubble's content box is the `MessageContent` element — the
      // first child div of the message wrapper.
      const userContent = userMessage.querySelector(
        ":scope > div",
      ) as HTMLElement;

      const messageRect = userMessage.getBoundingClientRect();
      const contentRect = userContent.getBoundingClientRect();

      // A two-character bubble must hug its text — strictly narrower than the
      // (95%-of-column) message wrapper.
      expect(contentRect.width).toBeLessThan(messageRect.width);
    } finally {
      cleanup();
    }
  });

  it("right-aligns the user action bar and left-aligns the assistant action bar", async () => {
    // The per-message action bar (`MessageActions`) is a column child of the
    // `Message` flex column. A user prompt bubble is right-aligned (`w-fit`,
    // `ml-auto`), so its copy/retry buttons must right-align too — the
    // `MessageActions` container carries `justify-end`. An assistant message is
    // left-aligned content, so its action bar stays at the default left
    // alignment (no `justify-end`). One send-prompt turn yields both a user
    // message and a streamed assistant reply, so both action bars are present.
    const harness = mockHarness({
      updates: [
        {
          sessionUpdate: "agent_message_chunk",
          content: textBlock("the assistant reply"),
        },
      ],
    });

    await renderInAct(
      <AiPanel
        boardDir="/tmp/board"
        models={MODELS}
        modelId="claude-code"
        onSelectModel={() => {}}
        onCollapse={() => {}}
        createConnect={harness.createConnect}
      />,
    );

    const textarea = screen.getByRole("textbox");
    await act(async () => {
      await userEvent.type(textarea, "a user prompt");
    });
    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: /submit/i }));
    });

    // Both messages render: the user prompt and the streamed assistant reply.
    await waitFor(() => {
      expect(document.body.textContent).toContain("a user prompt");
    });
    await waitFor(() => {
      expect(document.body.textContent).toContain("the assistant reply");
    });

    // The user message's action bar (the `MessageActions` div wrapping its copy
    // button) is right-aligned.
    const userMessage = document.querySelector(".is-user") as HTMLElement;
    const userCopy = within(userMessage).getByRole("button", {
      name: /copy message/i,
    });
    const userActionBar = userCopy.closest("div") as HTMLElement;
    expect(userActionBar.classList.contains("justify-end")).toBe(true);

    // The assistant message's action bar stays left-aligned (no `justify-end`).
    const assistantMessage = document.querySelector(
      ".is-assistant",
    ) as HTMLElement;
    const assistantCopy = within(assistantMessage).getByRole("button", {
      name: /copy message/i,
    });
    const assistantActionBar = assistantCopy.closest("div") as HTMLElement;
    expect(assistantActionBar.classList.contains("justify-end")).toBe(false);
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
        onCollapse={() => {}}
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

  it("never renders a 'New conversation' button in the composer", async () => {
    // Regression guard: the in-composer reset button was removed — the only
    // supported reset path is now the `ai.newChat` command. The button must
    // not appear on an empty conversation NOR after a message has streamed.
    const harness = mockHarness({
      updates: [
        {
          sessionUpdate: "agent_message_chunk",
          content: textBlock("a streamed reply"),
        },
      ],
    });

    await renderInAct(
      <AiPanel
        boardDir="/tmp/board"
        models={MODELS}
        modelId="claude-code"
        onSelectModel={() => {}}
        onCollapse={() => {}}
        createConnect={harness.createConnect}
      />,
    );

    // Empty panel: no reset button.
    expect(
      screen.queryByRole("button", { name: /new conversation/i }),
    ).toBeNull();

    // Send a prompt so the conversation becomes non-empty.
    const textarea = screen.getByRole("textbox");
    await act(async () => {
      await userEvent.type(textarea, "first prompt");
    });
    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: /submit/i }));
    });
    await waitFor(() => {
      expect(document.body.textContent).toContain("a streamed reply");
    });

    // Non-empty panel: still no reset button.
    expect(
      screen.queryByRole("button", { name: /new conversation/i }),
    ).toBeNull();
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
        onCollapse={() => {}}
        createConnect={harness.createConnect}
      />,
    );

    // The model selector now lives in the composer footer — its trigger is
    // a `role="combobox"` showing the selected model's label.
    await act(async () => {
      await userEvent.click(
        screen.getByRole("combobox", { name: /claude code/i }),
      );
    });

    const listbox = await screen.findByRole("listbox");
    const items = within(listbox).getAllByRole("option");
    expect(items).toHaveLength(2);

    const claude = items[0];
    const qwen = items[1];
    expect(claude.textContent).toContain("Claude Code");
    // The unavailable local model is disabled and surfaces its hint.
    expect(qwen.getAttribute("aria-disabled")).toBe("true");
    expect(qwen.textContent).toContain("Model weights unavailable");
  });

  it("renders the model selector in the composer, not the panel header", async () => {
    const harness = mockHarness();

    const { container } = await renderInAct(
      <AiPanel
        boardDir="/tmp/board"
        models={MODELS}
        modelId="claude-code"
        onSelectModel={() => {}}
        onCollapse={() => {}}
        createConnect={harness.createConnect}
      />,
    );

    const trigger = screen.getByRole("combobox", { name: /claude code/i });

    // The selector trigger is NOT inside the `<header>` — the header keeps
    // only the single AI-star collapse button.
    const header = document.querySelector("header");
    expect(header, "the panel header must be present").not.toBeNull();
    expect(
      within(header as HTMLElement).queryByRole("combobox"),
      "the panel header must no longer contain a model selector",
    ).toBeNull();
    expect(
      header!.contains(trigger),
      "the model selector must not be inside the header",
    ).toBe(false);

    // The header carries no "AI" text label — the star icon stands alone.
    expect(
      within(header as HTMLElement).queryByText("AI"),
      "the panel header must not render an 'AI' text label",
    ).toBeNull();
    // The single header button is the star-toggle that collapses the panel.
    const headerButtons = within(header as HTMLElement).getAllByRole("button");
    expect(
      headerButtons,
      "the panel header must contain exactly one button (the star toggle)",
    ).toHaveLength(1);
    const starCollapse = within(header as HTMLElement).getByRole("button", {
      name: /collapse ai panel/i,
    });
    expect(
      starCollapse.querySelector(".lucide-sparkles"),
      "the collapse button must use the sparkles icon",
    ).not.toBeNull();

    // It IS inside the composer region.
    const composer = container.querySelector(
      "[data-slot='ai-prompt-composer']",
    ) as HTMLElement | null;
    expect(composer, "the composer must be present").not.toBeNull();
    expect(
      composer!.contains(trigger),
      "the model selector must live inside the composer",
    ).toBe(true);
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
        onCollapse={() => {}}
        createConnect={harness.createConnect}
      />,
    );

    await act(async () => {
      await userEvent.click(
        screen.getByRole("combobox", { name: /claude code/i }),
      );
    });
    const listbox = await screen.findByRole("listbox");
    await act(async () => {
      await userEvent.click(
        within(listbox).getByRole("option", { name: /qwen coder/i }),
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
          onCollapse={() => {}}
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
        onCollapse={() => {}}
        createConnect={harness.createConnect}
      />,
    );

    // The composer is a CodeMirror 6 instance, not a `<textarea>`. A disabled
    // composer's CM6 content DOM is non-editable (`contenteditable="false"`)
    // rather than carrying the form-control `disabled` attribute.
    expect(screen.getByRole("textbox").getAttribute("contenteditable")).toBe(
      "false",
    );
  });
});

describe("AiPanel: board switch starts a fresh session", () => {
  it("changing boardDir while modelId stays the same remounts the conversation and re-invokes createConnect", async () => {
    // Regression: switching the active kanban board must tear down the prior
    // ACP client + session even when the model id is unchanged. The fix keys
    // `AiPanelConversation` on `${boardDir}::${modelId}` so a board change
    // unmounts/remounts the conversation, which (a) freshly initializes the
    // hook's `clientRef` / `sessionRef`, and (b) re-invokes `createConnect`.
    //
    // The production analogue: `useProductionConnect(boardDir)` hands the
    // panel a NEW factory each time the board changes, so production also
    // sees a fresh connect call. The test mirrors that by passing a fresh
    // `createConnect` for each board, but the load-bearing behavior is the
    // remount itself — without the composite key, the cached refs would
    // short-circuit and the new factory would never be called.
    const harnessA = mockHarness({
      updates: [
        { sessionUpdate: "agent_message_chunk", content: textBlock("from a") },
      ],
    });
    const harnessB = mockHarness({
      updates: [
        { sessionUpdate: "agent_message_chunk", content: textBlock("from b") },
      ],
    });

    const { rerender } = await renderInAct(
      <AiPanel
        boardDir="/a"
        models={MODELS}
        modelId="claude-code"
        onSelectModel={() => {}}
        onCollapse={() => {}}
        createConnect={harnessA.createConnect}
      />,
    );

    // Send one prompt against board /a — the first connect+session is built.
    const textareaA = screen.getByRole("textbox");
    await act(async () => {
      await userEvent.type(textareaA, "first");
    });
    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: /submit/i }));
    });
    await waitFor(() => {
      expect(harnessA.connectedModels()).toEqual(["claude-code"]);
    });
    expect(harnessA.sessions()).toHaveLength(1);
    const sessionA = harnessA.sessions()[0];

    // Switch the board (same model). The composite key flips, so
    // `AiPanelConversation` is unmounted — taking its `useConversation`
    // refs with it — and a fresh one mounts.
    await act(async () => {
      rerender(
        <AiPanel
          boardDir="/b"
          models={MODELS}
          modelId="claude-code"
          onSelectModel={() => {}}
          onCollapse={() => {}}
          createConnect={harnessB.createConnect}
        />,
      );
    });

    // The empty-state placeholder is back — proof the conversation was
    // remounted, since the prior prompt's user message would otherwise still
    // be in the message log.
    await waitFor(() => {
      expect(document.body.textContent).toContain(
        "Send a message to start the conversation",
      );
    });
    expect(document.body.textContent).not.toContain("first");

    // A new prompt against board /b triggers a fresh connect on the new
    // harness — proof the new `createConnect` factory was invoked, not the
    // cached client from the prior board.
    const textareaB = screen.getByRole("textbox");
    await act(async () => {
      await userEvent.type(textareaB, "second");
    });
    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: /submit/i }));
    });
    await waitFor(() => {
      expect(harnessB.connectedModels()).toEqual(["claude-code"]);
    });
    expect(harnessB.sessions()).toHaveLength(1);
    // The board-A session is preserved (no extra connect, no extra session),
    // and the board-B session is brand new — not the same object.
    expect(harnessA.sessions()).toHaveLength(1);
    expect(harnessB.sessions()[0]).not.toBe(sessionA);
  });

  it("re-rendering with the same boardDir + modelId does not remount the conversation", async () => {
    // The flip side: when neither the board nor the model changes, the
    // composite key is stable and `AiPanelConversation` must NOT remount.
    // The cached client + session survive so the existing conversation is
    // preserved across an unrelated prop change.
    const harness = mockHarness({
      updates: [
        { sessionUpdate: "agent_message_chunk", content: textBlock("ok") },
      ],
    });
    const { rerender } = await renderInAct(
      <AiPanel
        boardDir="/a"
        models={MODELS}
        modelId="claude-code"
        onSelectModel={() => {}}
        onCollapse={() => {}}
        createConnect={harness.createConnect}
      />,
    );

    const textarea = screen.getByRole("textbox");
    await act(async () => {
      await userEvent.type(textarea, "hello");
    });
    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: /submit/i }));
    });
    await waitFor(() => {
      expect(harness.sessions()).toHaveLength(1);
    });
    const session = harness.sessions()[0];

    // Re-render with the same board, same model, and the same
    // `createConnect`. The composite key is unchanged.
    await act(async () => {
      rerender(
        <AiPanel
          boardDir="/a"
          models={MODELS}
          modelId="claude-code"
          onSelectModel={() => {}}
          onCollapse={() => {}}
          createConnect={harness.createConnect}
        />,
      );
    });

    // Send another prompt — it must reuse the existing session, not start a
    // fresh one.
    await act(async () => {
      await userEvent.type(textarea, "again");
    });
    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: /submit/i }));
    });

    await waitFor(() => {
      expect(session.prompts.length).toBe(2);
    });
    expect(harness.sessions()).toHaveLength(1);
    expect(harness.sessions()[0]).toBe(session);
    // `createConnect` was invoked exactly once — only at the first
    // `ensureSession`. The same-key rerender did not retrigger it.
    expect(harness.connectedModels()).toEqual(["claude-code"]);
  });
});

describe("AiPanel: collapse control", () => {
  it("renders the collapse button in the header and clicking it calls onCollapse", async () => {
    const harness = mockHarness();
    const onCollapse = vi.fn();

    await renderInAct(
      <AiPanel
        boardDir="/tmp/board"
        models={MODELS}
        modelId="claude-code"
        onSelectModel={() => {}}
        onCollapse={onCollapse}
        createConnect={harness.createConnect}
      />,
    );

    // The collapse control lives in the panel header — `<header>` is the
    // single header row. After the AI/star consolidation it carries no "AI"
    // text and exactly one button: the sparkles-icon star toggle. The button
    // keeps the exact `aria-label` it had in the old standalone shell row.
    const header = document.querySelector("header");
    expect(header, "the panel header must be present").not.toBeNull();
    expect(
      within(header as HTMLElement).queryByText("AI"),
      "the panel header must not render an 'AI' text label",
    ).toBeNull();
    const headerButtons = within(header as HTMLElement).getAllByRole("button");
    expect(
      headerButtons,
      "the panel header must contain exactly one button (the star toggle)",
    ).toHaveLength(1);
    const collapse = within(header as HTMLElement).getByRole("button", {
      name: /collapse ai panel/i,
    });
    expect(
      collapse.querySelector(".lucide-sparkles"),
      "the collapse button must use the sparkles icon",
    ).not.toBeNull();

    await act(async () => {
      await userEvent.click(collapse);
    });

    expect(onCollapse).toHaveBeenCalledTimes(1);
  });
});

describe("AiPanel: slash-command autocomplete from availableCommands", () => {
  it("a streamed available_commands_update drives the composer's `/` menu", async () => {
    // The agent advertises its slash commands via `available_commands_update`;
    // `useConversation` folds them into `state.availableCommands`, which the
    // panel threads down to the composer's `/` autocomplete.
    const harness = mockHarness({
      updates: [
        {
          sessionUpdate: "available_commands_update",
          availableCommands: [
            { name: "plan", description: "Draft an execution plan" },
            { name: "review", description: "Review the diff" },
          ],
        },
        { sessionUpdate: "agent_message_chunk", content: textBlock("ok") },
      ],
    });

    const { container } = await renderInAct(
      <AiPanel
        boardDir="/tmp/board"
        models={MODELS}
        modelId="claude-code"
        onSelectModel={() => {}}
        onCollapse={() => {}}
        createConnect={harness.createConnect}
      />,
    );

    // Send a prompt so the fake session streams the available_commands_update.
    const textarea = screen.getByRole("textbox");
    await act(async () => {
      await userEvent.type(textarea, "hello");
    });
    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: /submit/i }));
    });
    await waitFor(() => {
      expect(document.body.textContent).toContain("ok");
    });

    // Type `/` in the (now-cleared) composer and assert the menu lists the
    // streamed commands — proof the availableCommands threaded all the way to
    // the composer's autocomplete.
    const { EditorView } = await import("@codemirror/view");
    const { completionStatus, currentCompletions } =
      await import("@codemirror/autocomplete");
    const cmEditor = container.querySelector(".cm-editor") as HTMLElement;
    const view = EditorView.findFromDOM(cmEditor)!;
    view.contentDOM.focus();

    await act(async () => {
      await userEvent.type(view.contentDOM, "/");
    });
    await waitFor(
      () => {
        expect(completionStatus(view.state)).toBe("active");
      },
      { timeout: 2000 },
    );

    const labels = currentCompletions(view.state).map((c) => c.label);
    expect(labels).toContain("/plan");
    expect(labels).toContain("/review");
  });

  it("warms up the session on model select so `/` works before any message", async () => {
    // The composer's `/` menu must work on a fresh conversation. The panel
    // warms up the session when the model is ready, so the agent's init
    // available_commands_update arrives without the user sending a message.
    const harness = mockHarness({
      initUpdates: [
        {
          sessionUpdate: "available_commands_update",
          availableCommands: [
            { name: "plan", description: "Draft an execution plan" },
            { name: "review", description: "Review the diff" },
          ],
        },
      ],
    });

    const { container } = await renderInAct(
      <AiPanel
        boardDir="/tmp/board"
        models={MODELS}
        modelId="claude-code"
        onSelectModel={() => {}}
        onCollapse={() => {}}
        createConnect={harness.createConnect}
      />,
    );

    // No prompt sent — the warm-up effect starts the session, which streams the
    // init available_commands_update.
    await waitFor(() => {
      expect(harness.sessions()).toHaveLength(1);
    });

    const { EditorView } = await import("@codemirror/view");
    const { completionStatus, currentCompletions } =
      await import("@codemirror/autocomplete");
    const cmEditor = container.querySelector(".cm-editor") as HTMLElement;
    const view = EditorView.findFromDOM(cmEditor)!;
    view.contentDOM.focus();

    await act(async () => {
      await userEvent.type(view.contentDOM, "/");
    });
    await waitFor(
      () => {
        expect(completionStatus(view.state)).toBe("active");
      },
      { timeout: 2000 },
    );

    const labels = currentCompletions(view.state).map((c) => c.label);
    expect(labels).toContain("/plan");
    expect(labels).toContain("/review");
    // The menu opened without any message being sent.
    expect(harness.sessions()[0].prompts).toEqual([]);
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
        onCollapse={() => {}}
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
        onCollapse={() => {}}
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

  it("re-warms the session after ai.newChat so `/` works in the fresh chat", async () => {
    // After a new chat resets the session, the composer's `/` menu must work
    // again without sending a message — the warm-up re-fires because the
    // conversation is empty again.
    const harness = mockHarness({
      initUpdates: [
        {
          sessionUpdate: "available_commands_update",
          availableCommands: [
            { name: "plan", description: "Draft an execution plan" },
          ],
        },
      ],
      updates: [
        { sessionUpdate: "agent_message_chunk", content: textBlock("a reply") },
      ],
    });

    const { container } = await renderInAct(
      <AiPanel
        boardDir="/tmp/board"
        models={MODELS}
        modelId="claude-code"
        onSelectModel={() => {}}
        onCollapse={() => {}}
        createConnect={harness.createConnect}
      />,
    );

    // Run a turn so the conversation is non-empty (warm-up #1 already ran).
    await act(async () => {
      await userEvent.type(screen.getByRole("textbox"), "hello");
    });
    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: /submit/i }));
    });
    await waitFor(() => {
      expect(document.body.textContent).toContain("a reply");
    });

    // New chat — drops the session and clears the log.
    await act(async () => {
      triggerAiNewChat();
    });
    await waitFor(() => {
      expect(document.body.textContent).not.toContain("a reply");
    });

    // The empty conversation re-warms: a fresh session starts (#2) and replays
    // the init available_commands_update — without any new message.
    await waitFor(() => {
      expect(harness.sessions().length).toBe(2);
    });

    const { EditorView } = await import("@codemirror/view");
    const { completionStatus, currentCompletions } =
      await import("@codemirror/autocomplete");
    const cmEditor = container.querySelector(".cm-editor") as HTMLElement;
    const view = EditorView.findFromDOM(cmEditor)!;
    view.contentDOM.focus();

    await act(async () => {
      await userEvent.type(view.contentDOM, "/");
    });
    await waitFor(
      () => {
        expect(completionStatus(view.state)).toBe("active");
      },
      { timeout: 2000 },
    );
    expect(currentCompletions(view.state).map((c) => c.label)).toContain(
      "/plan",
    );
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
        onCollapse={() => {}}
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

/**
 * The inline elicitation prompt — the sibling of the permission prompt.
 *
 * When the agent calls `unstable_createElicitation`, `useConversation` surfaces
 * the request and the panel renders an `ElicitationPrompt`: the agent's message,
 * the `ElicitationFields` form (form mode) or a link (url mode), and the
 * Submit / Decline / Cancel actions. These tests prime the connection (a first
 * prompt captures the elicitation handler), fire a request through the captured
 * handler, drive the rendered form, and assert the {@link CreateElicitationResponse}
 * the agent receives.
 */
describe("AiPanel: elicitation prompt", () => {
  /** A form-mode request with a required text field and an optional select. */
  function formRequest(): CreateElicitationRequest {
    return {
      sessionId: "fake-session",
      mode: "form",
      message: "Tell me about the deploy",
      requestedSchema: {
        type: "object",
        properties: {
          summary: { type: "string", title: "Summary" },
          severity: {
            type: "string",
            title: "Severity",
            enum: ["low", "high"],
          },
        },
        required: ["summary"],
      },
    };
  }

  /** A url-mode request directing the user to an external page. */
  function urlRequest(): CreateElicitationRequest {
    return {
      sessionId: "fake-session",
      mode: "url",
      message: "Authorize the integration",
      url: "https://example.com/authorize",
      elicitationId: "elicit-1",
    };
  }

  /**
   * The rendered elicitation prompt card.
   *
   * Scopes button/field queries to the prompt so they never collide with the
   * composer's own `Submit` button (the composer's send control shares that
   * accessible name).
   */
  function elicitationPrompt(): HTMLElement {
    const card = document.querySelector<HTMLElement>(
      "[data-slot='ai-elicitation-prompt']",
    );
    if (!card) {
      throw new Error("elicitation prompt is not rendered");
    }
    return card;
  }

  /**
   * Render the panel and send a first prompt so the connection — and thus the
   * elicitation handler — is captured. Returns the harness for the test to
   * fire elicitation requests through.
   */
  async function primedPanel(): Promise<MockHarness> {
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
        onCollapse={() => {}}
        createConnect={harness.createConnect}
      />,
    );
    const textarea = screen.getByRole("textbox");
    await act(async () => {
      await userEvent.type(textarea, "prime the connection");
    });
    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: /submit/i }));
    });
    return harness;
  }

  it("renders the form and a valid submit returns the typed accept content", async () => {
    const harness = await primedPanel();

    let outcome: Promise<CreateElicitationResponse> | undefined;
    await act(async () => {
      outcome = harness.elicitation()(formRequest());
    });

    // The agent's message and the field labels render.
    await waitFor(() => {
      expect(document.body.textContent).toContain("Tell me about the deploy");
    });
    expect(screen.getByLabelText(/summary/i)).toBeInTheDocument();

    // Fill the required text field, then submit.
    await act(async () => {
      await userEvent.type(screen.getByLabelText(/summary/i), "all green");
    });
    await act(async () => {
      await userEvent.click(
        within(elicitationPrompt()).getByRole("button", { name: /submit/i }),
      );
    });

    // The agent receives an `accept` whose content matches the schema — the
    // optional, untouched select is omitted entirely.
    await expect(outcome).resolves.toEqual({
      action: "accept",
      content: { summary: "all green" },
    });
    // The prompt is dismissed once answered.
    await waitFor(() => {
      expect(document.body.textContent).not.toContain(
        "Tell me about the deploy",
      );
    });
  });

  it("a missing required field blocks submit and shows an error", async () => {
    const harness = await primedPanel();

    let outcome: Promise<CreateElicitationResponse> | undefined;
    let settled = false;
    await act(async () => {
      outcome = harness.elicitation()(formRequest());
      void outcome.then(() => {
        settled = true;
      });
    });

    await waitFor(() => {
      expect(document.body.textContent).toContain("Tell me about the deploy");
    });

    // Submit with the required field left empty.
    await act(async () => {
      await userEvent.click(
        within(elicitationPrompt()).getByRole("button", { name: /submit/i }),
      );
    });

    // An error renders and the agent's request is NOT resolved — the prompt
    // stays on screen.
    await waitFor(() => {
      expect(document.body.textContent).toContain("summary is required");
    });
    expect(settled).toBe(false);
    expect(document.body.textContent).toContain("Tell me about the deploy");
  });

  it("Decline sends a decline action", async () => {
    const harness = await primedPanel();

    let outcome: Promise<CreateElicitationResponse> | undefined;
    await act(async () => {
      outcome = harness.elicitation()(formRequest());
    });
    await waitFor(() => {
      expect(document.body.textContent).toContain("Tell me about the deploy");
    });

    await act(async () => {
      await userEvent.click(
        within(elicitationPrompt()).getByRole("button", { name: /decline/i }),
      );
    });

    await expect(outcome).resolves.toEqual({ action: "decline" });
  });

  it("Cancel sends a cancel action", async () => {
    const harness = await primedPanel();

    let outcome: Promise<CreateElicitationResponse> | undefined;
    await act(async () => {
      outcome = harness.elicitation()(formRequest());
    });
    await waitFor(() => {
      expect(document.body.textContent).toContain("Tell me about the deploy");
    });

    await act(async () => {
      await userEvent.click(
        within(elicitationPrompt()).getByRole("button", { name: /cancel/i }),
      );
    });

    await expect(outcome).resolves.toEqual({ action: "cancel" });
  });

  it("url mode renders a link and no form fields", async () => {
    const harness = await primedPanel();

    await act(async () => {
      void harness.elicitation()(urlRequest());
    });

    await waitFor(() => {
      expect(document.body.textContent).toContain("Authorize the integration");
    });

    // The link points at the agent's url; no form fields are invented.
    const link = screen.getByRole("link", { name: /authorize/i });
    expect(link.getAttribute("href")).toBe("https://example.com/authorize");
    expect(
      document.querySelector("[data-slot='elicitation-fields']"),
      "url mode must render no form fields",
    ).toBeNull();
  });
});
