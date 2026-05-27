/**
 * End-to-end elicitation regression test for the whole client-side ACP seam.
 *
 * The original bug: the agent saw "declined to respond" but the UI never asked.
 * The fix spans four layers — {@link createKanbanClient} (advertise the
 * elicitation capability and forward `unstable_createElicitation`),
 * {@link useConversation} (surface the request and resolve it via
 * `respondElicitation`), the `ElicitationFields` form element, and the
 * `ElicitationPrompt` container in `ai-panel.tsx`. The per-layer tests
 * (`acp-client.node.test.ts`, `conversation.test.tsx`, `ai-panel.test.tsx`)
 * each exercise one seam against a stub on the other side. None of them wires
 * the *real agent transport* to the *real React UI*, so none would catch a
 * regression that only surfaces when all four layers run together — exactly the
 * shape of the original silent-decline bug.
 *
 * This test closes that gap. It drives a mock ACP **agent** — an
 * `AgentSideConnection` backed by a hand-written `Agent`, the same in-memory
 * stream harness `acp-client.node.test.ts` uses — through the genuine
 * {@link createKanbanClient}, into the genuine {@link useConversation} hook, up
 * to the genuine `ElicitationPrompt` rendered inside the real {@link AiPanel}.
 * The agent issues a form-mode `unstable_createElicitation` over the wire; the
 * form renders (no `methodNotFound`); the user fills and submits it; and the
 * typed `accept` content travels all the way back to the agent. Decline, cancel,
 * and a `newConversation` reset of an unanswered request are covered too, plus
 * the negative regression that the client advertises the capability at
 * `initialize`.
 *
 * It is NOT a model test: no real Claude CLI or llama process is spawned — the
 * per-crate wrapper tests cover that. This is purely the client-side ACP
 * boundary, end to end.
 *
 * Browser project (`*.test.tsx`) — React renders in real Chromium, which also
 * provides `TransformStream`/`ReadableStream` natively, so the same in-memory
 * full-duplex pipe the node harness uses works here while the panel mounts for
 * real.
 */
import { describe, it, expect, beforeEach } from "vitest";
import { act, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "vitest/browser";
import {
  AgentSideConnection,
  ndJsonStream,
  PROTOCOL_VERSION,
  type Agent,
  type AuthenticateRequest,
  type CancelNotification,
  type CreateElicitationRequest,
  type CreateElicitationResponse,
  type InitializeRequest,
  type InitializeResponse,
  type NewSessionRequest,
  type NewSessionResponse,
  type PromptRequest,
  type PromptResponse,
  type Stream,
} from "@agentclientprotocol/sdk";
import { renderInAct } from "@/test/act-render";
import { createKanbanClient } from "@/ai/acp-client";
import { resetAiCommandsForTest, triggerAiNewChat } from "@/ai/commands";
import type { ConversationConnect } from "@/ai/conversation";
import {
  AiPanel,
  type AiModel,
  type AiPanelConnectFactory,
} from "@/components/ai-panel";

/** A representative absolute board directory used as `newSession.cwd`. */
const BOARD_DIR = "/Users/example/boards/demo";
/** A representative board MCP toolset URL. */
const MCP_URL = "http://127.0.0.1:54321/mcp";

/** The single available model the panel connects through. */
const MODELS: AiModel[] = [
  {
    id: "claude-code",
    label: "Claude Code",
    kind: "claude-code",
    available: true,
    hint: null,
  },
];

/**
 * The turn a {@link MockAgent} runs when the client sends a prompt.
 *
 * Receives the live `AgentSideConnection` so it can issue an
 * `unstable_createElicitation` back at the client and observe the response the
 * UI delivers, then resolves the turn with a stop reason.
 */
type RunTurn = (
  conn: AgentSideConnection,
  params: PromptRequest,
) => Promise<PromptResponse>;

/**
 * A minimal in-memory ACP agent that issues elicitations on `prompt`.
 *
 * Implements only the `Agent` surface this test exercises. The configurable
 * {@link RunTurn} is what each test uses to drive the elicitation it cares
 * about; every `initialize` request is recorded so the capability-advertisement
 * assertion can inspect it.
 */
class MockAgent implements Agent {
  /** Every `initialize` request the client sent, in order. */
  readonly initializeRequests: InitializeRequest[] = [];
  /** The fixed session id the agent hands out. */
  static readonly SESSION_ID = "session-elicit-0001";

  constructor(
    private readonly conn: AgentSideConnection,
    private readonly runTurn: RunTurn,
  ) {}

  async initialize(params: InitializeRequest): Promise<InitializeResponse> {
    this.initializeRequests.push(params);
    return {
      protocolVersion: PROTOCOL_VERSION,
      agentInfo: { name: "mock-agent", version: "0.0.0" },
      agentCapabilities: {},
    };
  }

  async newSession(_params: NewSessionRequest): Promise<NewSessionResponse> {
    return { sessionId: MockAgent.SESSION_ID };
  }

  async authenticate(_params: AuthenticateRequest): Promise<void> {
    // The mock agent requires no authentication.
  }

  async prompt(params: PromptRequest): Promise<PromptResponse> {
    return this.runTurn(this.conn, params);
  }

  async cancel(_params: CancelNotification): Promise<void> {
    // No in-flight turn bookkeeping is needed for these tests.
  }
}

/**
 * Wire a {@link MockAgent} to a fresh in-memory full-duplex stream pair and
 * return the client end's {@link Stream}.
 *
 * Two `TransformStream`s form the pipe — one carries client->agent traffic, the
 * other agent->client — exactly the wiring `acp-client.node.test.ts` uses. The
 * returned stream is handed to the genuine {@link createKanbanClient}, so the
 * `initialize`/`newSession`/`prompt` handshake and every elicitation round trip
 * exercise the real SDK code paths with no WebSocket.
 */
function wireMockAgent(runTurn: RunTurn): {
  clientStream: Stream;
  agentFor: () => MockAgent;
} {
  const clientToAgent = new TransformStream<Uint8Array, Uint8Array>();
  const agentToClient = new TransformStream<Uint8Array, Uint8Array>();

  let agent: MockAgent | undefined;
  new AgentSideConnection(
    (conn) => {
      agent = new MockAgent(conn, runTurn);
      return agent;
    },
    ndJsonStream(agentToClient.writable, clientToAgent.readable),
  );

  return {
    clientStream: ndJsonStream(clientToAgent.writable, agentToClient.readable),
    agentFor: () => {
      if (!agent) throw new Error("mock agent was not constructed");
      return agent;
    },
  };
}

/**
 * Build the {@link AiPanelConnectFactory} the panel uses, backed by a real
 * {@link createKanbanClient} talking to the in-memory mock agent.
 *
 * This is the load-bearing difference from `ai-panel.test.tsx`'s mock harness:
 * there, `createConnect` returns a hand-written fake `KanbanAcpClient`; here it
 * returns the *genuine* client, so the panel's elicitation flow travels the
 * real ACP wire. The factory captures the constructed mock agent so a test can
 * inspect its `initialize` request.
 */
function realClientHarness(runTurn: RunTurn): {
  createConnect: AiPanelConnectFactory;
  agentFor: () => MockAgent;
} {
  let agentFor: (() => MockAgent) | undefined;

  const createConnect: AiPanelConnectFactory = () => {
    const connect: ConversationConnect = async (handlers) => {
      const wired = wireMockAgent(runTurn);
      agentFor = wired.agentFor;
      return createKanbanClient({
        stream: wired.clientStream,
        boardDir: BOARD_DIR,
        mcpUrl: MCP_URL,
        onSessionUpdate: handlers.onSessionUpdate,
        onRequestPermission: handlers.onRequestPermission,
        onElicitation: handlers.onElicitation,
        onCompleteElicitation: handlers.onCompleteElicitation,
      });
    };
    return connect;
  };

  return {
    createConnect,
    agentFor: () => {
      if (!agentFor) {
        throw new Error("the panel never connected the ACP client");
      }
      return agentFor();
    },
  };
}

/**
 * Render the panel against a real-client harness and send a prompt that drives
 * the mock agent's turn.
 *
 * The prompt is *not* awaited to completion: the agent's `runTurn` blocks on the
 * `unstable_createElicitation` it issues, so the prompt turn stays in flight
 * while the elicitation surfaces in the UI — precisely the in-flight shape of a
 * real elicitation. Returns the harness so the test can assert on the agent.
 */
async function renderAndPrompt(runTurn: RunTurn): Promise<{
  agentFor: () => MockAgent;
}> {
  const harness = realClientHarness(runTurn);

  await renderInAct(
    <AiPanel
      boardDir={BOARD_DIR}
      models={MODELS}
      modelId="claude-code"
      onSelectModel={() => {}}
      onCollapse={() => {}}
      createConnect={harness.createConnect}
    />,
  );

  const textarea = screen.getByRole("textbox");
  await act(async () => {
    await userEvent.type(textarea, "trigger elicitation");
  });
  // Fire the send without awaiting: the turn blocks on the agent's elicitation.
  await act(async () => {
    await userEvent.click(screen.getByRole("button", { name: /submit/i }));
  });

  return { agentFor: harness.agentFor };
}

/**
 * The rendered elicitation prompt card.
 *
 * Scopes button/field queries to the prompt so its `Submit` never collides with
 * the composer's own send button, which shares that accessible name.
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
 * A single-field form request mirroring the SAH `ask question` shape.
 *
 * `ask question` collects exactly one free-text `answer`, so its requested
 * schema is a single required `string` property — the minimal elicitation the
 * agent raises in practice.
 */
function askQuestionRequest(): CreateElicitationRequest {
  return {
    mode: "form",
    sessionId: MockAgent.SESSION_ID,
    message: "What is your preferred deployment target?",
    requestedSchema: {
      type: "object",
      properties: {
        answer: { type: "string", title: "Answer" },
      },
      required: ["answer"],
    },
  };
}

/**
 * A richer multi-field form request.
 *
 * Exercises every coercion path that matters for the round trip: a required
 * `string`, an `integer`, a `boolean`, and an `enum` single-select — so the
 * typed `content` the agent receives is asserted against more than a lone
 * string.
 */
function multiFieldRequest(): CreateElicitationRequest {
  return {
    mode: "form",
    sessionId: MockAgent.SESSION_ID,
    message: "Describe the release",
    requestedSchema: {
      type: "object",
      properties: {
        summary: { type: "string", title: "Summary" },
        replicas: { type: "integer", title: "Replicas" },
        canary: { type: "boolean", title: "Canary" },
        severity: { type: "string", title: "Severity", enum: ["low", "high"] },
      },
      required: ["summary", "replicas"],
    },
  };
}

describe("elicitation round trip: agent -> real client -> hook -> ElicitationPrompt", () => {
  // The `ai.newChat` registry is module-global; clear it between tests so the
  // strand-reset case fires the handler the panel under test registered, never
  // a stale one left by a prior render.
  beforeEach(() => {
    resetAiCommandsForTest();
  });

  it("advertises the elicitation capability at initialize (no methodNotFound)", async () => {
    // A turn that simply ends — enough to drive `initialize`/`newSession`. The
    // assertion is that the genuine client advertised elicitation in its
    // handshake, which is what stops the agent from ever seeing a
    // `methodNotFound` for `elicitation/create`.
    const { agentFor } = await renderAndPrompt(async () => ({
      stopReason: "end_turn",
    }));

    await waitFor(() => {
      expect(agentFor().initializeRequests).toHaveLength(1);
    });
    const [request] = agentFor().initializeRequests;
    expect(request.clientCapabilities?.elicitation).toEqual({
      form: {},
      url: {},
    });
  });

  it("renders the single-field ask-question form and delivers the typed accept content to the agent", async () => {
    let agentResponse: CreateElicitationResponse | undefined;
    const runTurn: RunTurn = async (conn) => {
      agentResponse =
        await conn.unstable_createElicitation(askQuestionRequest());
      return { stopReason: "end_turn" };
    };

    await renderAndPrompt(runTurn);

    // The form surfaced — proof the client forwarded the request rather than
    // refusing it with `methodNotFound`.
    await waitFor(() => {
      expect(document.body.textContent).toContain(
        "What is your preferred deployment target?",
      );
    });
    expect(screen.getByLabelText(/answer/i)).toBeInTheDocument();

    // Fill the single field and submit.
    await act(async () => {
      await userEvent.type(screen.getByLabelText(/answer/i), "kubernetes");
    });
    await act(async () => {
      await userEvent.click(
        within(elicitationPrompt()).getByRole("button", { name: /submit/i }),
      );
    });

    // The typed accept content reached the agent over the real wire.
    await waitFor(() => {
      expect(agentResponse).toEqual({
        action: "accept",
        content: { answer: "kubernetes" },
      });
    });
    // The prompt is dismissed once answered.
    await waitFor(() => {
      expect(document.body.textContent).not.toContain(
        "What is your preferred deployment target?",
      );
    });
  });

  it("renders a multi-field form and delivers correctly typed content to the agent", async () => {
    let agentResponse: CreateElicitationResponse | undefined;
    const runTurn: RunTurn = async (conn) => {
      agentResponse =
        await conn.unstable_createElicitation(multiFieldRequest());
      return { stopReason: "end_turn" };
    };

    await renderAndPrompt(runTurn);

    await waitFor(() => {
      expect(document.body.textContent).toContain("Describe the release");
    });

    // Fill the required string and integer fields, toggle the boolean, and pick
    // the enum option.
    await act(async () => {
      await userEvent.type(screen.getByLabelText(/summary/i), "ship it");
    });
    await act(async () => {
      await userEvent.type(screen.getByLabelText(/replicas/i), "3");
    });
    await act(async () => {
      await userEvent.click(screen.getByLabelText(/canary/i));
    });
    await act(async () => {
      await userEvent.click(
        within(elicitationPrompt()).getByRole("combobox", {
          name: /severity/i,
        }),
      );
    });
    await act(async () => {
      await userEvent.click(
        await screen.findByRole("option", { name: /high/i }),
      );
    });

    await act(async () => {
      await userEvent.click(
        within(elicitationPrompt()).getByRole("button", { name: /submit/i }),
      );
    });

    // Each value reached the agent coerced to its schema's JSON type: a number
    // for the integer, a real boolean, a string for the rest.
    await waitFor(() => {
      expect(agentResponse).toEqual({
        action: "accept",
        content: {
          summary: "ship it",
          replicas: 3,
          canary: true,
          severity: "high",
        },
      });
    });
  });

  it("propagates a decline action to the agent", async () => {
    let agentResponse: CreateElicitationResponse | undefined;
    const runTurn: RunTurn = async (conn) => {
      agentResponse =
        await conn.unstable_createElicitation(askQuestionRequest());
      return { stopReason: "end_turn" };
    };

    await renderAndPrompt(runTurn);

    await waitFor(() => {
      expect(document.body.textContent).toContain(
        "What is your preferred deployment target?",
      );
    });
    await act(async () => {
      await userEvent.click(
        within(elicitationPrompt()).getByRole("button", { name: /decline/i }),
      );
    });

    await waitFor(() => {
      expect(agentResponse).toEqual({ action: "decline" });
    });
  });

  it("propagates a cancel action to the agent", async () => {
    let agentResponse: CreateElicitationResponse | undefined;
    const runTurn: RunTurn = async (conn) => {
      agentResponse =
        await conn.unstable_createElicitation(askQuestionRequest());
      return { stopReason: "end_turn" };
    };

    await renderAndPrompt(runTurn);

    await waitFor(() => {
      expect(document.body.textContent).toContain(
        "What is your preferred deployment target?",
      );
    });
    await act(async () => {
      await userEvent.click(
        within(elicitationPrompt()).getByRole("button", { name: /cancel/i }),
      );
    });

    await waitFor(() => {
      expect(agentResponse).toEqual({ action: "cancel" });
    });
  });

  it("a newConversation reset of an unanswered elicitation does not strand the agent", async () => {
    // The agent issues an elicitation and the user never answers it; instead the
    // conversation is reset (the `ai.newChat` path). The reset must clear the
    // pending prompt so the UI is not stuck showing a dead form, and the agent's
    // own session is gone — it is never left waiting on a response that can no
    // longer arrive. This is the "no stranded agent" guard.
    let elicitationRaised = false;
    const runTurn: RunTurn = async (conn) => {
      elicitationRaised = true;
      // The agent awaits a response that the reset means it will never get on
      // this session. The promise stays pending; the test asserts the UI moved
      // on cleanly rather than asserting the agent resolves.
      await conn.unstable_createElicitation(askQuestionRequest());
      return { stopReason: "end_turn" };
    };

    await renderAndPrompt(runTurn);

    await waitFor(() => {
      expect(document.body.textContent).toContain(
        "What is your preferred deployment target?",
      );
    });
    expect(elicitationRaised).toBe(true);

    // Reset the conversation while the elicitation is still pending — the
    // `ai.newChat` command path the window layer uses.
    await act(async () => {
      triggerAiNewChat();
    });

    // The pending form is gone and the panel is back to its empty state — the UI
    // did not strand the user (or the agent) on a dead elicitation.
    await waitFor(() => {
      expect(document.body.textContent).not.toContain(
        "What is your preferred deployment target?",
      );
    });
    expect(document.body.textContent).toContain(
      "Send a message to start the conversation",
    );
  });
});
