/**
 * The kanban app's ACP client — pure TypeScript, running in the webview.
 *
 * This module owns the client end of the Agent Client Protocol: it wraps the
 * `@agentclientprotocol/sdk` `ClientSideConnection`, supplies a {@link Client}
 * implementation for every agent->client method, and drives the session
 * lifecycle (`initialize` -> `newSession` -> `prompt`/`cancel`) against the
 * in-process ACP agent.
 *
 * # Where the pieces come from
 *
 * - The {@link Stream} is the WebSocket-backed message stream from
 *   {@link connectAcpStream} (`acp-stream.ts`). The ACP traffic travels over
 *   that loopback `ws://` socket — Tauri IPC is never on the data path.
 * - The agent's `ws://` URL and the board's `mcpUrl` both come from the
 *   one-time `ai_start_agent` Tauri command. `mcpUrl` is handed to the agent
 *   verbatim as the HTTP `McpServer` entry in `newSession`, giving the agent
 *   the board's full SwissArmyHammer toolset.
 *
 * # Honest capabilities
 *
 * v1 does **not** advertise filesystem or terminal capabilities. The agent
 * does all file and shell work through the SAH MCP toolset, not through
 * client-side ACP methods. So {@link createKanbanClient}'s `initialize` sends
 * `fs: { readTextFile: false, writeTextFile: false }` and omits `terminal`,
 * and the corresponding {@link Client} methods are deliberate refusals — an
 * agent that ignores the advertised capabilities and calls them anyway gets a
 * clear `method not found` error rather than a silent hang.
 *
 * # Stateless
 *
 * Nothing is persisted. Every chat is a fresh {@link AcpSession} backed by its
 * own `newSession`; closing the panel drops the session and, via
 * {@link connectAcpStream}'s `close`, the underlying socket.
 */
import {
  ClientSideConnection,
  PROTOCOL_VERSION,
  RequestError,
  type Agent,
  type Client,
  type Stream,
  type CancelNotification,
  type CompleteElicitationNotification,
  type ContentBlock,
  type CreateElicitationRequest,
  type CreateTerminalRequest,
  type InitializeResponse,
  type KillTerminalRequest,
  type McpServer,
  type PromptResponse,
  type ReadTextFileRequest,
  type ReleaseTerminalRequest,
  type RequestPermissionRequest,
  type RequestPermissionResponse,
  type SessionNotification,
  type SetSessionModeResponse,
  type TerminalOutputRequest,
  type WaitForTerminalExitRequest,
  type WriteTextFileRequest,
} from "@agentclientprotocol/sdk";

/**
 * Client identity sent to the agent in `initialize`.
 *
 * Purely informational — the agent may surface it for debugging or metrics.
 */
const CLIENT_INFO = {
  name: "swissarmyhammer-kanban",
  title: "SwissArmyHammer Kanban",
  version: "0.1.0",
} as const;

/**
 * Handler invoked for every `session/update` notification the agent sends.
 *
 * The conversation store is owned by a later task; this client does not
 * invent it. The caller injects a handler that forwards updates wherever they
 * belong (a store, a reducer, a test spy). The handler runs to completion
 * before the client acknowledges the notification.
 */
export type SessionUpdateHandler = (
  params: SessionNotification,
) => void | Promise<void>;

/**
 * Handler invoked when the agent requests user permission for a tool call.
 *
 * The kanban UI presents {@link RequestPermissionRequest.options} to the user
 * and resolves with their decision. Returning a `cancelled` outcome is valid —
 * e.g. when the surrounding prompt turn was cancelled.
 */
export type RequestPermissionHandler = (
  params: RequestPermissionRequest,
) => Promise<RequestPermissionResponse>;

/** Dependencies injected into {@link createKanbanClient}. */
export interface KanbanClientOptions {
  /** The ACP message stream — typically from {@link connectAcpStream}. */
  stream: Stream;
  /**
   * Absolute path of the open board's directory. Becomes `newSession.cwd`,
   * the base the agent resolves relative paths against.
   */
  boardDir: string;
  /**
   * The board's full-SAH-toolset MCP URL — the `mcpUrl` returned by
   * `ai_start_agent`. Sent as the HTTP {@link McpServerHttp} entry in
   * `newSession.mcpServers`. When the board has no MCP server (`ai_start_agent`
   * returned `null`), pass `null` and `newSession` carries no MCP servers.
   */
  mcpUrl: string | null;
  /** Forwards `session/update` notifications to the conversation store. */
  onSessionUpdate: SessionUpdateHandler;
  /** Resolves `session/request_permission` requests via the UI. */
  onRequestPermission: RequestPermissionHandler;
}

/**
 * A live ACP chat session: one `newSession` and the operations that act on it.
 *
 * Obtained from {@link KanbanAcpClient.startSession}. A session is stateless
 * and disposable — start a fresh one per chat rather than reusing it.
 */
export interface AcpSession {
  /** The agent-assigned session id, used for every operation below. */
  readonly sessionId: string;
  /**
   * Send a user prompt and await the completed turn.
   *
   * `session/update` notifications stream to {@link SessionUpdateHandler}
   * while the turn runs; this promise resolves once the agent reports a stop
   * reason.
   *
   * @param prompt - The user message content blocks.
   * @returns The turn's {@link PromptResponse}, including its `stopReason`.
   */
  prompt(prompt: ContentBlock[]): Promise<PromptResponse>;
  /**
   * Cancel the session's in-flight prompt turn.
   *
   * A `session/cancel` notification — fire-and-forget. The corresponding
   * {@link AcpSession.prompt} promise then resolves with a `cancelled` stop
   * reason once the agent winds down.
   */
  cancel(): Promise<void>;
  /**
   * Switch the session's operational mode (e.g. "ask" vs "code").
   *
   * The mode must be one the agent advertised in `newSession`'s `modes`.
   * Optional — only call it when the agent reports available modes.
   *
   * @param modeId - The id of the mode to activate.
   */
  setMode(modeId: string): Promise<SetSessionModeResponse | void>;
}

/**
 * The kanban app's connected ACP client.
 *
 * Wraps a `ClientSideConnection` whose `initialize` handshake has already
 * completed and its protocol version verified. Use {@link startSession} to
 * begin a chat.
 */
export interface KanbanAcpClient {
  /** The protocol version negotiated with the agent at `initialize`. */
  readonly protocolVersion: number;
  /** The agent's `initialize` response — capabilities, auth methods, info. */
  readonly initializeResponse: InitializeResponse;
  /**
   * Open a fresh, stateless chat session.
   *
   * Issues `newSession` with the board directory as `cwd` and the board's
   * MCP toolset as the sole HTTP MCP server. Each call is independent — start
   * one per chat.
   */
  startSession(): Promise<AcpSession>;
}

/**
 * Raised when the agent negotiates an ACP protocol version this client does
 * not support.
 *
 * The SDK and the Rust agent crate version independently, so a mismatch is a
 * real possibility on upgrade. Surfacing a typed, clearly-worded error lets
 * the UI tell the user to update rather than failing deep inside a later
 * `newSession` or `prompt` call with an opaque message.
 */
export class AcpProtocolVersionError extends Error {
  /** The protocol version this client offered (and supports). */
  readonly clientVersion: number;
  /** The protocol version the agent negotiated in its `initialize` reply. */
  readonly agentVersion: number;

  constructor(clientVersion: number, agentVersion: number) {
    super(
      `ACP protocol version mismatch: this client supports version ` +
        `${clientVersion}, but the agent negotiated version ${agentVersion}. ` +
        `Update the kanban app or the AI agent so their ACP versions match.`,
    );
    this.name = "AcpProtocolVersionError";
    this.clientVersion = clientVersion;
    this.agentVersion = agentVersion;
  }
}

/**
 * Build the {@link Client} implementation handed to `ClientSideConnection`.
 *
 * Every agent->client method is implemented — none is left to fall through to
 * the SDK's default `method not found`:
 *
 * - `sessionUpdate` / `requestPermission` are forwarded to the injected
 *   handlers — the genuine integration points with the UI and the store.
 * - The filesystem and terminal methods are *deliberate refusals*. v1 does not
 *   advertise `fs` or `terminal` capabilities (see this module's docstring),
 *   so a well-behaved agent never calls them. Should one try anyway, it gets a
 *   clear `RequestError.methodNotFound` instead of a silent hang — exactly the
 *   response the SDK would synthesize for an unimplemented optional method,
 *   but explicit and self-documenting here.
 * - The `unstable_` elicitation methods are likewise *deliberate refusals*. v1
 *   advertises no elicitation capability, so a well-behaved agent never calls
 *   them; implementing them explicitly makes this `Client` enumerate the full
 *   SDK interface and self-document the v1 stance.
 * - `extMethod` / `extNotification` likewise refuse: this client defines no
 *   ACP extensions.
 */
function buildClient(
  onSessionUpdate: SessionUpdateHandler,
  onRequestPermission: RequestPermissionHandler,
): Client {
  /**
   * Reject an agent->client call for a capability v1 does not advertise.
   *
   * `methodNotFound` is the honest answer: from the agent's perspective the
   * method is unavailable because its backing capability was never offered.
   */
  const refuseCapability = (method: string): never => {
    throw RequestError.methodNotFound(method);
  };

  return {
    async sessionUpdate(params: SessionNotification): Promise<void> {
      await onSessionUpdate(params);
    },

    requestPermission(
      params: RequestPermissionRequest,
    ): Promise<RequestPermissionResponse> {
      return onRequestPermission(params);
    },

    // Filesystem methods — refused. The `fs` capability is not advertised in
    // `initialize`; the agent reads and writes files through the SAH MCP
    // toolset instead.
    readTextFile(_params: ReadTextFileRequest): Promise<never> {
      return Promise.reject(refuseCapability("fs/read_text_file"));
    },
    writeTextFile(_params: WriteTextFileRequest): Promise<never> {
      return Promise.reject(refuseCapability("fs/write_text_file"));
    },

    // Terminal methods — refused. The `terminal` capability is not advertised;
    // the agent runs shell commands through the SAH MCP toolset instead.
    createTerminal(_params: CreateTerminalRequest): Promise<never> {
      return Promise.reject(refuseCapability("terminal/create"));
    },
    terminalOutput(_params: TerminalOutputRequest): Promise<never> {
      return Promise.reject(refuseCapability("terminal/output"));
    },
    releaseTerminal(_params: ReleaseTerminalRequest): Promise<never> {
      return Promise.reject(refuseCapability("terminal/release"));
    },
    waitForTerminalExit(_params: WaitForTerminalExitRequest): Promise<never> {
      return Promise.reject(refuseCapability("terminal/wait_for_exit"));
    },
    killTerminal(_params: KillTerminalRequest): Promise<never> {
      return Promise.reject(refuseCapability("terminal/kill"));
    },

    // Elicitation methods (`unstable_`, experimental) — refused. v1 advertises
    // no elicitation capability, so a well-behaved agent never calls these.
    unstable_createElicitation(
      _params: CreateElicitationRequest,
    ): Promise<never> {
      return Promise.reject(refuseCapability("elicitation/create"));
    },
    unstable_completeElicitation(
      _params: CompleteElicitationNotification,
    ): Promise<void> {
      // A notification — like `extNotification`, it expects no response, so an
      // unsupported elicitation-complete is silently dropped rather than
      // erroring a fire-and-forget message.
      return Promise.resolve();
    },

    // Extension methods — refused. This client defines no ACP extensions.
    extMethod(method: string): Promise<never> {
      return Promise.reject(refuseCapability(`_ext/${method}`));
    },
    extNotification(_method: string): Promise<void> {
      // Notifications expect no response; an unknown extension notification is
      // silently dropped rather than erroring a fire-and-forget message.
      return Promise.resolve();
    },
  };
}

/**
 * Build the HTTP MCP server entry for the board's SAH toolset.
 *
 * Returns the single-element list `newSession.mcpServers` carries, or an empty
 * list when the board exposes no MCP server. The entry is the ACP
 * `McpServer` `"http"` variant — the board's loopback `http://…/mcp` URL with
 * no auth headers.
 */
function mcpServersFor(mcpUrl: string | null): McpServer[] {
  if (mcpUrl === null) {
    return [];
  }
  return [
    {
      type: "http",
      name: "swissarmyhammer-kanban",
      url: mcpUrl,
      // No auth headers: the MCP server is a loopback endpoint scoped to the
      // open board.
      headers: [],
    },
  ];
}

/**
 * Wrap a `ClientSideConnection` as an {@link AcpSession} bound to one session.
 *
 * `prompt`, `cancel`, and `setMode` are thin adapters that inject the captured
 * `sessionId` so callers never thread it through by hand.
 */
function makeSession(agent: Agent, sessionId: string): AcpSession {
  return {
    sessionId,
    prompt(prompt: ContentBlock[]): Promise<PromptResponse> {
      return agent.prompt({ sessionId, prompt });
    },
    cancel(): Promise<void> {
      const notification: CancelNotification = { sessionId };
      return agent.cancel(notification);
    },
    setMode(modeId: string): Promise<SetSessionModeResponse | void> {
      if (!agent.setSessionMode) {
        return Promise.reject(
          new Error("the agent does not support setting session modes"),
        );
      }
      return agent.setSessionMode({ sessionId, modeId });
    },
  };
}

/**
 * Connect and initialize an ACP client over `stream`.
 *
 * Drives the full client-side handshake:
 *
 * 1. Constructs a `ClientSideConnection` with a {@link Client} that forwards
 *    `sessionUpdate`/`requestPermission` to the injected handlers and refuses
 *    the unadvertised fs/terminal capabilities.
 * 2. Sends `initialize` with honest capabilities — `fs` read/write `false`,
 *    `terminal` omitted.
 * 3. Verifies the negotiated protocol version: any version other than the one
 *    this client offered rejects with {@link AcpProtocolVersionError} so the
 *    mismatch surfaces immediately and clearly.
 *
 * The returned {@link KanbanAcpClient} can then open stateless chat sessions
 * via {@link KanbanAcpClient.startSession}.
 *
 * @param options - The stream, board directory, MCP URL, and the two handlers.
 * @returns A connected, initialized {@link KanbanAcpClient}.
 * @throws AcpProtocolVersionError when the agent negotiates an unsupported
 *   protocol version.
 */
export async function createKanbanClient(
  options: KanbanClientOptions,
): Promise<KanbanAcpClient> {
  const { stream, boardDir, mcpUrl, onSessionUpdate, onRequestPermission } =
    options;

  const client = buildClient(onSessionUpdate, onRequestPermission);
  const connection = new ClientSideConnection(() => client, stream);

  const initializeResponse = await connection.initialize({
    protocolVersion: PROTOCOL_VERSION,
    clientInfo: CLIENT_INFO,
    // Honest capabilities: v1 does not do client-side files or shell. The
    // agent uses the SAH MCP toolset for both.
    clientCapabilities: {
      fs: { readTextFile: false, writeTextFile: false },
    },
  });

  // The agent echoes the version it agreed to. Anything other than the
  // version this client offered is a genuine incompatibility — the SDK and
  // the Rust agent crate version independently — so fail loudly now rather
  // than deep inside a later `newSession` or `prompt`.
  if (initializeResponse.protocolVersion !== PROTOCOL_VERSION) {
    throw new AcpProtocolVersionError(
      PROTOCOL_VERSION,
      initializeResponse.protocolVersion,
    );
  }

  return {
    protocolVersion: initializeResponse.protocolVersion,
    initializeResponse,
    async startSession(): Promise<AcpSession> {
      const { sessionId } = await connection.newSession({
        cwd: boardDir,
        mcpServers: mcpServersFor(mcpUrl),
      });
      return makeSession(connection, sessionId);
    },
  };
}
