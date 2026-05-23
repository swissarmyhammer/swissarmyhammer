/**
 * Hook-level tests for {@link useConversation}.
 *
 * `conversation.node.test.ts` exhaustively covers the pure reducers; this file
 * covers the React shell: the {@link useConversation} hook driving a fake ACP
 * client through `sendPrompt`, `cancel`, `newConversation`, and the permission
 * round trip. It asserts the hook exposes exactly the surface the AI panel
 * needs and that the surface behaves.
 *
 * Browser project (`*.test.tsx`) — `renderHook` needs a React renderer. The
 * ACP client is a hand-written fake implementing the `KanbanAcpClient` shape;
 * the genuine protocol plumbing is tested in `acp-client.node.test.ts`.
 */
import { describe, it, expect } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import type {
  CompleteElicitationNotification,
  ContentBlock,
  CreateElicitationRequest,
  CreateElicitationResponse,
  PromptResponse,
  RequestPermissionRequest,
  RequestPermissionResponse,
  SessionNotification,
} from "@agentclientprotocol/sdk";
import type {
  AcpSession,
  CompleteElicitationHandler,
  ElicitationHandler,
  KanbanAcpClient,
  RequestPermissionHandler,
  SessionUpdateHandler,
} from "./acp-client";
import { useConversation, type ConversationConnect } from "./conversation";

/** A plain ACP text content block. */
function textBlock(text: string): ContentBlock {
  return { type: "text", text };
}

/**
 * A controllable fake ACP session.
 *
 * `prompt` streams the configured notifications to the captured
 * `onSessionUpdate` handler, then resolves with the configured stop reason —
 * the same observable shape as a real turn, with no transport.
 */
class FakeSession implements AcpSession {
  readonly sessionId = "fake-session";
  /** Every prompt the hook sent, in order. */
  readonly prompts: ContentBlock[][] = [];
  /** Whether `cancel` was called. */
  cancelled = false;

  constructor(
    private readonly onUpdate: SessionUpdateHandler,
    private readonly script: {
      updates?: SessionNotification["update"][];
      stopReason?: PromptResponse["stopReason"];
      reject?: boolean;
    },
  ) {}

  async prompt(prompt: ContentBlock[]): Promise<PromptResponse> {
    this.prompts.push(prompt);
    if (this.script.reject) {
      throw new Error("prompt failed");
    }
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
 * Build a {@link ConversationConnect} factory backed by a {@link FakeSession}.
 *
 * Returns the factory plus a getter for the constructed session, so a test can
 * both drive the hook and inspect what the fake agent received.
 */
function fakeConnect(script: {
  updates?: SessionNotification["update"][];
  /**
   * Ambient `session/update`s streamed during `startSession` — before any
   * prompt — mirroring how the real claude backend forwards the CLI's slash
   * commands as an `available_commands_update` at session start.
   */
  initUpdates?: SessionNotification["update"][];
  stopReason?: PromptResponse["stopReason"];
  reject?: boolean;
}): {
  connect: ConversationConnect;
  sessions: () => FakeSession[];
  permission: () => RequestPermissionHandler;
  elicitation: () => ElicitationHandler;
  completeElicitation: () => CompleteElicitationHandler;
} {
  const sessions: FakeSession[] = [];
  let capturedPermission: RequestPermissionHandler | undefined;
  let capturedElicitation: ElicitationHandler | undefined;
  let capturedCompleteElicitation: CompleteElicitationHandler | undefined;

  const connect: ConversationConnect = async (handlers) => {
    capturedPermission = handlers.onRequestPermission;
    capturedElicitation = handlers.onElicitation;
    capturedCompleteElicitation = handlers.onCompleteElicitation;
    const client: KanbanAcpClient = {
      protocolVersion: 1,
      initializeResponse: {
        protocolVersion: 1,
        agentCapabilities: {},
      },
      async startSession(): Promise<AcpSession> {
        const session = new FakeSession(handlers.onSessionUpdate, script);
        sessions.push(session);
        // Replay any init-time ambient updates (e.g. available_commands_update)
        // exactly as the real agent does during `session/new`.
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

  return {
    connect,
    sessions: () => sessions,
    permission: () => {
      if (!capturedPermission) {
        throw new Error("connect was never invoked");
      }
      return capturedPermission;
    },
    elicitation: () => {
      if (!capturedElicitation) {
        throw new Error("connect was never invoked");
      }
      return capturedElicitation;
    },
    completeElicitation: () => {
      if (!capturedCompleteElicitation) {
        throw new Error("connect was never invoked");
      }
      return capturedCompleteElicitation;
    },
  };
}

/** A minimal form-mode elicitation request the agent might raise. */
function formElicitation(message: string): CreateElicitationRequest {
  return {
    mode: "form",
    sessionId: "fake-session",
    requestedSchema: { properties: {} },
    message,
  };
}

describe("useConversation", () => {
  it("exposes exactly the documented hook surface", () => {
    const { connect } = fakeConnect({});
    const { result } = renderHook(() => useConversation({ connect }));

    expect(Object.keys(result.current).sort()).toEqual(
      [
        "cancel",
        "elicitationRequest",
        "messages",
        "newConversation",
        "permissionRequest",
        "respondElicitation",
        "respondPermission",
        "sendPrompt",
        "state",
        "status",
        "warmUp",
      ].sort(),
    );
    expect(result.current.status).toBe("idle");
    expect(result.current.messages).toEqual([]);
    expect(result.current.permissionRequest).toBeNull();
    expect(result.current.elicitationRequest).toBeNull();
  });

  it("warmUp starts the session and folds the init available_commands_update into state without a prompt", async () => {
    const { connect, sessions } = fakeConnect({
      initUpdates: [
        {
          sessionUpdate: "available_commands_update",
          availableCommands: [{ name: "plan", description: "Plan the work" }],
        },
      ],
    });
    const { result } = renderHook(() => useConversation({ connect }));

    await act(async () => {
      result.current.warmUp();
    });

    // The session started — without any prompt — and the agent's init
    // available_commands_update folded into state.
    await waitFor(() => {
      expect(sessions()).toHaveLength(1);
    });
    await waitFor(() => {
      expect(result.current.state.availableCommands).toEqual([
        { name: "plan", description: "Plan the work" },
      ]);
    });
    expect(sessions()[0].prompts).toEqual([]);
    expect(result.current.messages).toEqual([]);
    expect(result.current.status).toBe("idle");
  });

  it("a warmUp racing a sendPrompt starts only one session", async () => {
    // The warm-up effect and a fast first send must share one session, not
    // spawn two agent processes.
    const { connect, sessions } = fakeConnect({
      updates: [
        { sessionUpdate: "agent_message_chunk", content: textBlock("ok") },
      ],
    });
    const { result } = renderHook(() => useConversation({ connect }));

    await act(async () => {
      result.current.warmUp();
      await result.current.sendPrompt([textBlock("hi")]);
    });

    await waitFor(() => {
      expect(sessions()).toHaveLength(1);
    });
    expect(sessions()[0].prompts).toEqual([[textBlock("hi")]]);
  });

  it("newConversation during an in-flight warm-up does not cache the abandoned session", async () => {
    // Race: a new chat (ai.newChat) fires while the warm-up's startSession is
    // still resolving. The abandoned start must NOT write its session back into
    // the cache — otherwise the "fresh" chat would silently reuse the prior
    // ACP session, breaking the brand-new-session-per-chat contract.
    let releaseFirstStart: (() => void) | undefined;
    let startCount = 0;
    const sessions: FakeSession[] = [];
    const connect: ConversationConnect = async (handlers) => ({
      protocolVersion: 1,
      initializeResponse: { protocolVersion: 1, agentCapabilities: {} },
      async startSession(): Promise<AcpSession> {
        startCount += 1;
        // Gate only the first start so we can interleave a reset while it is
        // in flight.
        if (startCount === 1) {
          await new Promise<void>((resolve) => {
            releaseFirstStart = resolve;
          });
        }
        const session = new FakeSession(handlers.onSessionUpdate, {});
        sessions.push(session);
        return session;
      },
    });
    const { result } = renderHook(() => useConversation({ connect }));

    // Warm up — the first startSession is now pending on the gate.
    await act(async () => {
      result.current.warmUp();
    });
    await waitFor(() => {
      expect(startCount).toBe(1);
    });

    // Reset mid-start, then release the abandoned first start.
    await act(async () => {
      result.current.newConversation();
    });
    await act(async () => {
      releaseFirstStart?.();
      await Promise.resolve();
    });

    // The next send must open a brand-new session (start #2), not reuse the
    // abandoned one. The abandoned session #0 is never used.
    await act(async () => {
      await result.current.sendPrompt([textBlock("hi")]);
    });
    expect(sessions).toHaveLength(2);
    expect(sessions[1].prompts).toEqual([[textBlock("hi")]]);
    expect(sessions[0].prompts).toEqual([]);
  });

  it("sendPrompt streams updates into messages and ends idle", async () => {
    const { connect, sessions } = fakeConnect({
      updates: [
        { sessionUpdate: "agent_message_chunk", content: textBlock("Hi ") },
        { sessionUpdate: "agent_message_chunk", content: textBlock("there") },
      ],
      stopReason: "end_turn",
    });
    const { result } = renderHook(() => useConversation({ connect }));

    await act(async () => {
      await result.current.sendPrompt([textBlock("hello")]);
    });

    expect(result.current.status).toBe("idle");
    // The user's message plus the streamed assistant reply.
    expect(result.current.messages).toHaveLength(2);
    expect(result.current.messages[0]).toMatchObject({ role: "user" });
    expect(result.current.messages[1]).toMatchObject({ role: "assistant" });
    expect(result.current.messages[1].parts[0]).toMatchObject({
      type: "text",
      text: "Hi there",
      state: "done",
    });
    expect(sessions()[0].prompts).toEqual([[textBlock("hello")]]);
  });

  it("sendPrompt lands in error when the turn rejects", async () => {
    const { connect } = fakeConnect({ reject: true });
    const { result } = renderHook(() => useConversation({ connect }));

    await act(async () => {
      await result.current.sendPrompt([textBlock("boom")]);
    });

    expect(result.current.status).toBe("error");
  });

  it("a refusal stop reason lands the turn in error", async () => {
    const { connect } = fakeConnect({ stopReason: "refusal" });
    const { result } = renderHook(() => useConversation({ connect }));

    await act(async () => {
      await result.current.sendPrompt([textBlock("forbidden")]);
    });

    expect(result.current.status).toBe("error");
  });

  it("cancel forwards to the live session", async () => {
    const { connect, sessions } = fakeConnect({ stopReason: "cancelled" });
    const { result } = renderHook(() => useConversation({ connect }));

    await act(async () => {
      await result.current.sendPrompt([textBlock("long task")]);
    });
    await act(async () => {
      await result.current.cancel();
    });

    expect(sessions()[0].cancelled).toBe(true);
  });

  it("newConversation clears the store and starts a fresh session", async () => {
    const { connect, sessions } = fakeConnect({
      updates: [
        { sessionUpdate: "agent_message_chunk", content: textBlock("reply") },
      ],
    });
    const { result } = renderHook(() => useConversation({ connect }));

    await act(async () => {
      await result.current.sendPrompt([textBlock("first")]);
    });
    expect(result.current.messages.length).toBeGreaterThan(0);

    act(() => {
      result.current.newConversation();
    });
    expect(result.current.messages).toEqual([]);
    expect(result.current.status).toBe("idle");

    await act(async () => {
      await result.current.sendPrompt([textBlock("second")]);
    });
    // A fresh, stateless session was started for the new conversation.
    expect(sessions()).toHaveLength(2);
  });

  it("tool_call followed by tool_call_update completed advances the tool part", async () => {
    // Regression coverage for the "AI panel tool calls never leave pending"
    // bug: the agent now forwards tool_result completions as
    // `tool_call_update` notifications and the hook must fold them onto the
    // matching pending tool part.
    const { connect } = fakeConnect({
      updates: [
        {
          sessionUpdate: "tool_call",
          toolCallId: "call-42",
          title: "search_board",
          kind: "search",
          status: "pending",
          rawInput: { query: "kanban" },
        },
        {
          sessionUpdate: "tool_call_update",
          toolCallId: "call-42",
          status: "completed",
          rawOutput: { hits: 3 },
        },
      ],
      stopReason: "end_turn",
    });
    const { result } = renderHook(() => useConversation({ connect }));

    await act(async () => {
      await result.current.sendPrompt([textBlock("hello")]);
    });

    // The user message plus the assistant message that owns the tool part.
    expect(result.current.messages).toHaveLength(2);
    const part = result.current.messages[1].parts[0];
    expect(part).toMatchObject({
      type: "dynamic-tool",
      toolCallId: "call-42",
      state: "output-available",
      input: { query: "kanban" },
      output: { hits: 3 },
    });
  });

  it("surfaces a permission request and resolves it via respondPermission", async () => {
    const { connect, permission } = fakeConnect({});
    const { result } = renderHook(() => useConversation({ connect }));

    // Prime the client connection so the permission handler is captured.
    await act(async () => {
      await result.current.sendPrompt([textBlock("trigger")]);
    });

    const request: RequestPermissionRequest = {
      sessionId: "fake-session",
      toolCall: {
        toolCallId: "call-1",
        title: "Edit config",
        kind: "edit",
        status: "pending",
      },
      options: [
        { kind: "allow_once", name: "Allow", optionId: "allow" },
        { kind: "reject_once", name: "Deny", optionId: "deny" },
      ],
    };

    let decision: Promise<RequestPermissionResponse> | undefined;
    act(() => {
      decision = permission()(request);
    });

    await waitFor(() => {
      expect(result.current.permissionRequest).toEqual(request);
    });

    act(() => {
      result.current.respondPermission({
        outcome: { outcome: "selected", optionId: "deny" },
      });
    });

    expect(result.current.permissionRequest).toBeNull();
    await expect(decision).resolves.toEqual({
      outcome: { outcome: "selected", optionId: "deny" },
    });
  });

  it("surfaces an elicitation request and resolves it via respondElicitation", async () => {
    const { connect, elicitation } = fakeConnect({});
    const { result } = renderHook(() => useConversation({ connect }));

    // Prime the client connection so the elicitation handler is captured.
    await act(async () => {
      await result.current.sendPrompt([textBlock("trigger")]);
    });

    const request = formElicitation("Confirm the destructive action?");

    let decision: Promise<CreateElicitationResponse> | undefined;
    act(() => {
      decision = elicitation()(request);
    });

    await waitFor(() => {
      expect(result.current.elicitationRequest).toEqual(request);
    });

    const response: CreateElicitationResponse = {
      action: "accept",
      content: { confirm: true },
    };
    act(() => {
      result.current.respondElicitation(response);
    });

    expect(result.current.elicitationRequest).toBeNull();
    await expect(decision).resolves.toEqual(response);
  });

  it("onCompleteElicitation clears the pending elicitation request", async () => {
    const { connect, elicitation, completeElicitation } = fakeConnect({});
    const { result } = renderHook(() => useConversation({ connect }));

    await act(async () => {
      await result.current.sendPrompt([textBlock("trigger")]);
    });

    act(() => {
      void elicitation()(formElicitation("Open the linked page"));
    });
    await waitFor(() => {
      expect(result.current.elicitationRequest).not.toBeNull();
    });

    const completion: CompleteElicitationNotification = {
      elicitationId: "elicit-1",
    };
    act(() => {
      completeElicitation()(completion);
    });

    expect(result.current.elicitationRequest).toBeNull();
  });

  it("newConversation clears a pending elicitation request", async () => {
    const { connect, elicitation } = fakeConnect({});
    const { result } = renderHook(() => useConversation({ connect }));

    await act(async () => {
      await result.current.sendPrompt([textBlock("trigger")]);
    });

    act(() => {
      void elicitation()(formElicitation("Provide a value"));
    });
    await waitFor(() => {
      expect(result.current.elicitationRequest).not.toBeNull();
    });

    act(() => {
      result.current.newConversation();
    });

    expect(result.current.elicitationRequest).toBeNull();
  });
});
