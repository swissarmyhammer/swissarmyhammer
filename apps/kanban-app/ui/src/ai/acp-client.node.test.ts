/**
 * Integration tests for the kanban app's ACP client.
 *
 * `acp-client.ts` is the client end of the Agent Client Protocol: it wraps the
 * SDK's `ClientSideConnection`, supplies a `Client` for every agent->client
 * method, and drives `initialize` -> `newSession` -> `prompt`/`cancel`.
 *
 * These tests run the real client against a *mock agent* — an
 * `AgentSideConnection` backed by a hand-written `Agent` — wired to the client
 * over an in-memory pair of `TransformStream`s. That is the same in-process
 * wiring the SDK's own `acp.test.ts` uses, so the handshake, the JSON-RPC
 * round trips, and the protocol-version negotiation all exercise the genuine
 * SDK code paths without a real WebSocket.
 *
 * Node-only (no DOM, no React) — pure protocol plumbing. Lives under the
 * `*.node.test.ts` suffix recognized by `vite.config.ts`.
 */
import { describe, it, expect } from "vitest";
import {
  AgentSideConnection,
  ndJsonStream,
  PROTOCOL_VERSION,
  type Agent,
  type AuthenticateRequest,
  type CancelNotification,
  type CompleteElicitationNotification,
  type CreateElicitationResponse,
  type InitializeRequest,
  type InitializeResponse,
  type NewSessionRequest,
  type NewSessionResponse,
  type PromptRequest,
  type PromptResponse,
  type SetSessionModeRequest,
  type SetSessionModeResponse,
  type Stream,
} from "@agentclientprotocol/sdk";
import {
  AcpProtocolVersionError,
  createKanbanClient,
  type CompleteElicitationHandler,
  type ElicitationHandler,
  type RequestPermissionHandler,
  type SessionUpdateHandler,
} from "./acp-client";

/** A representative absolute board directory used as `newSession.cwd`. */
const BOARD_DIR = "/Users/example/boards/demo";
/** A representative board MCP toolset URL. */
const MCP_URL = "http://127.0.0.1:54321/mcp";

/**
 * Knobs for {@link MockAgent}: overrides that let a single mock cover every
 * test scenario — the happy path, the permission round trip, and the
 * version-mismatch path.
 */
interface MockAgentOptions {
  /** Protocol version the agent negotiates in `initialize`. */
  protocolVersion?: number;
  /** Whether the agent advertises (and implements) `setSessionMode`. */
  supportSessionMode?: boolean;
  /**
   * The turn the agent runs on `prompt`. Receives the `AgentSideConnection`
   * so it can stream `sessionUpdate`s and call `requestPermission` back at
   * the client. Defaults to a no-op turn that ends normally.
   */
  runTurn?: (
    conn: AgentSideConnection,
    params: PromptRequest,
  ) => Promise<PromptResponse>;
}

/**
 * A minimal in-memory ACP agent for driving the client under test.
 *
 * Implements only the `Agent` surface the client exercises. Every
 * `newSession` request is recorded on {@link MockAgent.newSessionRequests} so
 * tests can assert what the client sent (notably the `mcpServers` entry).
 */
class MockAgent implements Agent {
  /** Every `newSession` request the client sent, in order. */
  readonly newSessionRequests: NewSessionRequest[] = [];
  /** Every `initialize` request the client sent, in order. */
  readonly initializeRequests: InitializeRequest[] = [];
  /** The `sessionId` the agent hands out — one fixed value is enough. */
  static readonly SESSION_ID = "session-0001";

  constructor(
    private readonly conn: AgentSideConnection,
    private readonly options: MockAgentOptions,
  ) {}

  async initialize(params: InitializeRequest): Promise<InitializeResponse> {
    this.initializeRequests.push(params);
    return {
      protocolVersion: this.options.protocolVersion ?? PROTOCOL_VERSION,
      agentInfo: { name: "mock-agent", version: "0.0.0" },
      agentCapabilities: {},
    };
  }

  async newSession(params: NewSessionRequest): Promise<NewSessionResponse> {
    this.newSessionRequests.push(params);
    return { sessionId: MockAgent.SESSION_ID };
  }

  async authenticate(_params: AuthenticateRequest): Promise<void> {
    // The mock agent requires no authentication.
  }

  async prompt(params: PromptRequest): Promise<PromptResponse> {
    const runTurn = this.options.runTurn;
    if (!runTurn) {
      return { stopReason: "end_turn" };
    }
    return runTurn(this.conn, params);
  }

  async cancel(_params: CancelNotification): Promise<void> {
    // No in-flight turn to abort in these synchronous mock turns.
  }

  async setSessionMode(
    _params: SetSessionModeRequest,
  ): Promise<SetSessionModeResponse | void> {
    if (!this.options.supportSessionMode) {
      throw new Error("session modes not supported");
    }
    return {};
  }
}

/**
 * Wire a {@link MockAgent} to a fresh pair of in-memory streams and return the
 * client end's {@link Stream}.
 *
 * Two `TransformStream`s form a full-duplex pipe: one carries client->agent
 * traffic, the other agent->client. The mock agent is constructed on its end;
 * the returned stream is handed to {@link createKanbanClient}.
 */
function wireMockAgent(options: MockAgentOptions = {}): {
  clientStream: Stream;
  agentFor: () => MockAgent;
} {
  const clientToAgent = new TransformStream<Uint8Array, Uint8Array>();
  const agentToClient = new TransformStream<Uint8Array, Uint8Array>();

  let agent: MockAgent | undefined;
  new AgentSideConnection(
    (conn) => {
      agent = new MockAgent(conn, options);
      return agent;
    },
    ndJsonStream(agentToClient.writable, clientToAgent.readable),
  );

  return {
    clientStream: ndJsonStream(clientToAgent.writable, agentToClient.readable),
    // The agent is created lazily when the SDK first routes a message; by the
    // time any assertion runs, `initialize` has constructed it.
    agentFor: () => {
      if (!agent) throw new Error("mock agent was not constructed");
      return agent;
    },
  };
}

/** A `sessionUpdate` handler that records every notification it receives. */
function recordingSessionUpdates(): {
  handler: SessionUpdateHandler;
  updates: unknown[];
} {
  const updates: unknown[] = [];
  return {
    updates,
    handler: (params) => {
      updates.push(params.update);
    },
  };
}

/** A `requestPermission` handler that always selects the first option. */
const selectFirstOption: RequestPermissionHandler = async (params) => ({
  outcome: { outcome: "selected", optionId: params.options[0].optionId },
});

/**
 * A default `onElicitation` handler used by tests that don't exercise
 * elicitation. It declines — a valid {@link CreateElicitationResponse} that
 * keeps the agent from hanging if a stray elicitation arrives.
 */
const declineElicitation: ElicitationHandler = async () => ({
  action: "decline",
});

/** A no-op `onCompleteElicitation` handler for tests that don't assert on it. */
const ignoreCompleteElicitation: CompleteElicitationHandler = () => {};

describe("createKanbanClient", () => {
  it("completes the initialize handshake with honest, fs/terminal-free capabilities", async () => {
    const { clientStream, agentFor } = wireMockAgent();
    const { handler } = recordingSessionUpdates();

    const client = await createKanbanClient({
      stream: clientStream,
      boardDir: BOARD_DIR,
      mcpUrl: MCP_URL,
      onSessionUpdate: handler,
      onRequestPermission: selectFirstOption,
      onElicitation: declineElicitation,
      onCompleteElicitation: ignoreCompleteElicitation,
    });

    expect(client.protocolVersion).toBe(PROTOCOL_VERSION);

    const [request] = agentFor().initializeRequests;
    expect(request.protocolVersion).toBe(PROTOCOL_VERSION);
    // fs is advertised as explicitly unsupported; terminal is omitted.
    expect(request.clientCapabilities?.fs).toEqual({
      readTextFile: false,
      writeTextFile: false,
    });
    expect(request.clientCapabilities?.terminal ?? false).toBe(false);
  });

  it("advertises form and url elicitation capability in the initialize handshake", async () => {
    const { clientStream, agentFor } = wireMockAgent();
    const { handler } = recordingSessionUpdates();

    await createKanbanClient({
      stream: clientStream,
      boardDir: BOARD_DIR,
      mcpUrl: MCP_URL,
      onSessionUpdate: handler,
      onRequestPermission: selectFirstOption,
      onElicitation: declineElicitation,
      onCompleteElicitation: ignoreCompleteElicitation,
    });

    const [request] = agentFor().initializeRequests;
    // Elicitation is now advertised: both form and url sub-capabilities so the
    // agent may drive either flavor.
    expect(request.clientCapabilities?.elicitation).toEqual({
      form: {},
      url: {},
    });
  });

  it("sends an HTTP mcpServers entry for the board toolset in newSession", async () => {
    const { clientStream, agentFor } = wireMockAgent();
    const { handler } = recordingSessionUpdates();

    const client = await createKanbanClient({
      stream: clientStream,
      boardDir: BOARD_DIR,
      mcpUrl: MCP_URL,
      onSessionUpdate: handler,
      onRequestPermission: selectFirstOption,
      onElicitation: declineElicitation,
      onCompleteElicitation: ignoreCompleteElicitation,
    });
    await client.startSession();

    const [newSession] = agentFor().newSessionRequests;
    expect(newSession.cwd).toBe(BOARD_DIR);
    expect(newSession.mcpServers).toHaveLength(1);
    expect(newSession.mcpServers[0]).toEqual({
      type: "http",
      name: "swissarmyhammer-kanban",
      url: MCP_URL,
      headers: [],
    });
  });

  it("sends an empty mcpServers list when the board has no MCP server", async () => {
    const { clientStream, agentFor } = wireMockAgent();
    const { handler } = recordingSessionUpdates();

    const client = await createKanbanClient({
      stream: clientStream,
      boardDir: BOARD_DIR,
      mcpUrl: null,
      onSessionUpdate: handler,
      onRequestPermission: selectFirstOption,
      onElicitation: declineElicitation,
      onCompleteElicitation: ignoreCompleteElicitation,
    });
    await client.startSession();

    expect(agentFor().newSessionRequests[0].mcpServers).toEqual([]);
  });

  it("runs a prompt round trip and forwards session updates to the store handler", async () => {
    const { clientStream } = wireMockAgent({
      async runTurn(conn, params) {
        await conn.sessionUpdate({
          sessionId: params.sessionId,
          update: {
            sessionUpdate: "agent_message_chunk",
            content: { type: "text", text: "Hello back!" },
          },
        });
        return { stopReason: "end_turn" };
      },
    });
    const { handler, updates } = recordingSessionUpdates();

    const client = await createKanbanClient({
      stream: clientStream,
      boardDir: BOARD_DIR,
      mcpUrl: MCP_URL,
      onSessionUpdate: handler,
      onRequestPermission: selectFirstOption,
      onElicitation: declineElicitation,
      onCompleteElicitation: ignoreCompleteElicitation,
    });
    const session = await client.startSession();

    const response = await session.prompt([
      { type: "text", text: "Hello, agent!" },
    ]);

    expect(response.stopReason).toBe("end_turn");
    expect(updates).toEqual([
      {
        sessionUpdate: "agent_message_chunk",
        content: { type: "text", text: "Hello back!" },
      },
    ]);
  });

  it("runs a requestPermission round trip and returns the user's selected option", async () => {
    let agentOutcome: unknown;
    const { clientStream } = wireMockAgent({
      async runTurn(conn, params) {
        const decision = await conn.requestPermission({
          sessionId: params.sessionId,
          toolCall: {
            toolCallId: "call_1",
            title: "Edit config.json",
            kind: "edit",
            status: "pending",
          },
          options: [
            { kind: "allow_once", name: "Allow", optionId: "allow" },
            { kind: "reject_once", name: "Skip", optionId: "reject" },
          ],
        });
        agentOutcome = decision.outcome;
        return { stopReason: "end_turn" };
      },
    });
    const { handler } = recordingSessionUpdates();

    // The injected UI handler picks the second option ("reject").
    const pickReject: RequestPermissionHandler = async (params) => ({
      outcome: { outcome: "selected", optionId: params.options[1].optionId },
    });

    const client = await createKanbanClient({
      stream: clientStream,
      boardDir: BOARD_DIR,
      mcpUrl: MCP_URL,
      onSessionUpdate: handler,
      onRequestPermission: pickReject,
      onElicitation: declineElicitation,
      onCompleteElicitation: ignoreCompleteElicitation,
    });
    const session = await client.startSession();
    await session.prompt([{ type: "text", text: "Change the config" }]);

    // The agent received exactly the option the injected handler chose.
    expect(agentOutcome).toEqual({ outcome: "selected", optionId: "reject" });
  });

  it("forwards unstable_createElicitation to the handler and returns an accept-with-content response", async () => {
    // The agent issues a form elicitation; the injected handler accepts with
    // user-provided content, and the agent must receive exactly that response.
    let agentResponse: CreateElicitationResponse | undefined;
    const { clientStream } = wireMockAgent({
      async runTurn(conn, params) {
        agentResponse = await conn.unstable_createElicitation({
          mode: "form",
          sessionId: params.sessionId,
          requestedSchema: {
            type: "object",
            properties: { name: { type: "string" } },
          },
          message: "What is your name?",
        });
        return { stopReason: "end_turn" };
      },
    });
    const { handler } = recordingSessionUpdates();

    let elicitationParams: unknown;
    const acceptWithName: ElicitationHandler = async (params) => {
      elicitationParams = params;
      return { action: "accept", content: { name: "Ada" } };
    };

    const client = await createKanbanClient({
      stream: clientStream,
      boardDir: BOARD_DIR,
      mcpUrl: MCP_URL,
      onSessionUpdate: handler,
      onRequestPermission: selectFirstOption,
      onElicitation: acceptWithName,
      onCompleteElicitation: ignoreCompleteElicitation,
    });
    const session = await client.startSession();
    await session.prompt([{ type: "text", text: "Trigger elicitation" }]);

    // The handler saw the request the agent sent.
    expect(elicitationParams).toMatchObject({
      mode: "form",
      message: "What is your name?",
    });
    // The agent received the handler's accept-with-content response.
    expect(agentResponse).toEqual({
      action: "accept",
      content: { name: "Ada" },
    });
  });

  it("forwards a declined elicitation response to the agent", async () => {
    let agentResponse: CreateElicitationResponse | undefined;
    const { clientStream } = wireMockAgent({
      async runTurn(conn, params) {
        agentResponse = await conn.unstable_createElicitation({
          mode: "url",
          sessionId: params.sessionId,
          elicitationId: "elic-1",
          url: "https://example.com/elicit",
          message: "More input needed",
        });
        return { stopReason: "end_turn" };
      },
    });
    const { handler } = recordingSessionUpdates();

    const client = await createKanbanClient({
      stream: clientStream,
      boardDir: BOARD_DIR,
      mcpUrl: MCP_URL,
      onSessionUpdate: handler,
      onRequestPermission: selectFirstOption,
      onElicitation: declineElicitation,
      onCompleteElicitation: ignoreCompleteElicitation,
    });
    const session = await client.startSession();
    await session.prompt([{ type: "text", text: "Trigger elicitation" }]);

    expect(agentResponse).toEqual({ action: "decline" });
  });

  it("forwards a cancelled elicitation response to the agent", async () => {
    let agentResponse: CreateElicitationResponse | undefined;
    const { clientStream } = wireMockAgent({
      async runTurn(conn, params) {
        agentResponse = await conn.unstable_createElicitation({
          mode: "url",
          sessionId: params.sessionId,
          elicitationId: "elic-1",
          url: "https://example.com/elicit",
          message: "More input needed",
        });
        return { stopReason: "end_turn" };
      },
    });
    const { handler } = recordingSessionUpdates();

    const cancelElicitation: ElicitationHandler = async () => ({
      action: "cancel",
    });

    const client = await createKanbanClient({
      stream: clientStream,
      boardDir: BOARD_DIR,
      mcpUrl: MCP_URL,
      onSessionUpdate: handler,
      onRequestPermission: selectFirstOption,
      onElicitation: cancelElicitation,
      onCompleteElicitation: ignoreCompleteElicitation,
    });
    const session = await client.startSession();
    await session.prompt([{ type: "text", text: "Trigger elicitation" }]);

    expect(agentResponse).toEqual({ action: "cancel" });
  });

  it("forwards unstable_completeElicitation to the completion handler", async () => {
    const completions: CompleteElicitationNotification[] = [];
    const { clientStream } = wireMockAgent({
      async runTurn(conn) {
        await conn.unstable_completeElicitation({ elicitationId: "elic-1" });
        return { stopReason: "end_turn" };
      },
    });
    const { handler } = recordingSessionUpdates();

    const recordCompletion: CompleteElicitationHandler = (params) => {
      completions.push(params);
    };

    const client = await createKanbanClient({
      stream: clientStream,
      boardDir: BOARD_DIR,
      mcpUrl: MCP_URL,
      onSessionUpdate: handler,
      onRequestPermission: selectFirstOption,
      onElicitation: declineElicitation,
      onCompleteElicitation: recordCompletion,
    });
    const session = await client.startSession();
    await session.prompt([{ type: "text", text: "Complete elicitation" }]);

    expect(completions).toEqual([{ elicitationId: "elic-1" }]);
  });

  it("cancels an in-flight prompt turn without throwing", async () => {
    const { clientStream } = wireMockAgent();
    const { handler } = recordingSessionUpdates();

    const client = await createKanbanClient({
      stream: clientStream,
      boardDir: BOARD_DIR,
      mcpUrl: MCP_URL,
      onSessionUpdate: handler,
      onRequestPermission: selectFirstOption,
      onElicitation: declineElicitation,
      onCompleteElicitation: ignoreCompleteElicitation,
    });
    const session = await client.startSession();

    // `cancel` is a fire-and-forget notification — it must resolve cleanly.
    await expect(session.cancel()).resolves.toBeUndefined();
  });

  it("drives setSessionMode when the agent supports it", async () => {
    const { clientStream } = wireMockAgent({ supportSessionMode: true });
    const { handler } = recordingSessionUpdates();

    const client = await createKanbanClient({
      stream: clientStream,
      boardDir: BOARD_DIR,
      mcpUrl: MCP_URL,
      onSessionUpdate: handler,
      onRequestPermission: selectFirstOption,
      onElicitation: declineElicitation,
      onCompleteElicitation: ignoreCompleteElicitation,
    });
    const session = await client.startSession();

    await expect(session.setMode("code")).resolves.not.toThrow();
  });

  it("surfaces a clear error when the agent negotiates an unsupported protocol version", async () => {
    // The agent answers `initialize` with a version this client never offered.
    const incompatibleVersion = PROTOCOL_VERSION + 99;
    const { clientStream } = wireMockAgent({
      protocolVersion: incompatibleVersion,
    });
    const { handler } = recordingSessionUpdates();

    const attempt = createKanbanClient({
      stream: clientStream,
      boardDir: BOARD_DIR,
      mcpUrl: MCP_URL,
      onSessionUpdate: handler,
      onRequestPermission: selectFirstOption,
      onElicitation: declineElicitation,
      onCompleteElicitation: ignoreCompleteElicitation,
    });

    await expect(attempt).rejects.toBeInstanceOf(AcpProtocolVersionError);
    await expect(attempt).rejects.toThrow(/protocol version mismatch/i);
    await expect(attempt).rejects.toMatchObject({
      clientVersion: PROTOCOL_VERSION,
      agentVersion: incompatibleVersion,
    });
  });
});
