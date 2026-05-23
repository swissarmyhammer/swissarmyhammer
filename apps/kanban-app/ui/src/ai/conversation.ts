/**
 * Conversation state derived from the ACP `session/update` notification stream.
 *
 * The ACP `sessionUpdate` stream IS the conversation source of truth. This
 * module folds that stream into renderable message state and exposes it as a
 * React hook ({@link useConversation}) that the AI Elements components consume
 * directly.
 *
 * # Why a purpose-built adapter, not a chat framework
 *
 * The AI SDK's `useChat` and TanStack AI exist to *talk to LLM providers* —
 * they own turns, streaming, and tool orchestration. The ACP agent and the
 * `ClientSideConnection` (see `acp-client.ts`) already do all of that. Routing
 * through a chat framework would mean two lossy adapters (ACP -> framework ->
 * `UIMessage`), an extra alpha dependency, and a message model that still is
 * not AI Elements' `UIMessage`. Worse, a generic chat hook cannot express
 * ACP's permission *requests*, plans, or session modes.
 *
 * So this module is the single, purpose-built adapter: ACP {@link SessionUpdate}
 * -> AI-SDK-`UIMessage`-shaped parts that AI Elements renders without further
 * translation.
 *
 * # The fold
 *
 * {@link applySessionUpdate} is a pure reducer: `(state, update) -> state`. It
 * has no React dependency, which is what makes the per-variant translation
 * exhaustively unit-testable. {@link useConversation} is the thin React shell
 * that owns the reducer state, wires it to the ACP client's injected
 * `sessionUpdate`/`requestPermission` callbacks, and tracks turn status.
 *
 * # Mapping summary
 *
 * | `SessionUpdate` variant       | Result                                          |
 * |-------------------------------|-------------------------------------------------|
 * | `user_message_chunk`          | user message, text part (chunks coalesce)       |
 * | `agent_message_chunk`         | assistant message, text part (chunks coalesce)  |
 * | `agent_thought_chunk`         | assistant message, reasoning part (coalesce)    |
 * | `tool_call`                   | assistant message, `dynamic-tool` part          |
 * | `tool_call_update`            | updates the matching `dynamic-tool` part        |
 * | `plan`                        | assistant message, `data-plan` part (replaced)  |
 * | `available_commands_update`   | {@link ConversationState.availableCommands}     |
 * | `current_mode_update`         | {@link ConversationState.currentModeId}         |
 * | `config_option_update`        | {@link ConversationState.configOptions}         |
 * | `session_info_update`         | {@link ConversationState.sessionInfo}           |
 * | `usage_update`                | {@link ConversationState.usage}                 |
 *
 * # Stateless
 *
 * Nothing is persisted. {@link ConversationApi.newConversation} clears the
 * store and starts a fresh stateless `newSession`, exactly as `acp-client.ts`
 * intends — every chat is a brand-new session.
 */
import { useCallback, useMemo, useReducer, useRef, useState } from "react";
import type {
  AvailableCommand,
  CompleteElicitationNotification,
  ContentBlock,
  CreateElicitationRequest,
  CreateElicitationResponse,
  PlanEntry,
  RequestPermissionRequest,
  RequestPermissionResponse,
  SessionConfigOption,
  SessionNotification,
  SessionUpdate,
  StopReason,
  ToolCallContent,
  ToolCallStatus,
  ToolKind,
} from "@agentclientprotocol/sdk";
import type {
  DynamicToolUIPart,
  ReasoningUIPart,
  TextUIPart,
  UIMessage,
} from "ai";
import type { AcpSession, KanbanAcpClient } from "./acp-client";

/**
 * The data payload of the custom `data-plan` UI part.
 *
 * AI Elements has no built-in `UIMessage` part for an ACP execution plan, so a
 * plan is carried as an AI SDK *data part* (`type: "data-plan"`). The panel
 * renders it with the `Task` family of AI Elements components. Carrying the
 * raw {@link PlanEntry} list keeps the part a faithful, lossless projection of
 * the ACP `plan` update.
 */
export interface PlanPartData {
  /** The plan's task entries, mirroring the ACP `Plan.entries` field. */
  entries: PlanEntry[];
}

/**
 * The `UIMessage` data-part map this adapter produces.
 *
 * Only `plan` is custom; every other part this module emits is a built-in AI
 * SDK part (`text`, `reasoning`, `dynamic-tool`). The index signature is what
 * satisfies the AI SDK's `UIDataTypes` (`Record<string, unknown>`) constraint
 * on `UIMessage`'s data-parts type parameter; in practice only `plan` is set.
 */
export type ConversationDataParts = {
  /** An ACP execution plan, rendered by the `Task` AI Elements components. */
  plan: PlanPartData;
} & Record<string, unknown>;

/**
 * A conversation message — an AI SDK {@link UIMessage} specialized to the
 * data-part union this adapter emits.
 *
 * `messages` is exposed as `ConversationMessage[]`, which *is* `UIMessage[]`,
 * so the AI Elements `Message`/`Tool`/`Reasoning`/`Task` components consume it
 * with no further translation.
 */
export type ConversationMessage = UIMessage<unknown, ConversationDataParts>;

/**
 * A message part of a {@link ConversationMessage}.
 *
 * Derived from {@link ConversationMessage} so it carries the exact same
 * data-parts and tools type parameters — keeping the two structurally
 * interchangeable. This adapter only ever emits `text`, `reasoning`,
 * `dynamic-tool`, and the custom `data-plan` parts.
 */
type ConversationPart = ConversationMessage["parts"][number];

/**
 * The state of the {@link ConversationStatus} state machine.
 *
 * - `idle` — no turn running; the conversation is awaiting user input.
 * - `streaming` — a prompt turn is in flight and updates are arriving.
 * - `error` — the last turn failed, or the agent refused (`refusal` stop
 *   reason). Cleared by the next {@link ConversationApi.sendPrompt}.
 */
export type ConversationStatus = "idle" | "streaming" | "error";

/**
 * The full conversation store: the message log plus the ambient session state
 * that has no natural home in a single message.
 *
 * `available_commands_update`, `current_mode_update`, `config_option_update`,
 * `session_info_update`, and `usage_update` describe the *session*, not one
 * message, so they live here as discrete fields rather than as message parts.
 */
export interface ConversationState {
  /** The message log, oldest first — directly renderable `UIMessage[]`. */
  messages: ConversationMessage[];
  /** Commands the agent currently offers, from `available_commands_update`. */
  availableCommands: AvailableCommand[];
  /** The agent's current session mode id, from `current_mode_update`. */
  currentModeId: string | null;
  /** The session's configuration options, from `config_option_update`. */
  configOptions: SessionConfigOption[];
  /** Human-readable session info, from `session_info_update`. */
  sessionInfo: SessionInfoState;
  /** Token/cost usage for the session, from `usage_update`. */
  usage: UsageState | null;
}

/** Session info tracked from `session_info_update`. */
export interface SessionInfoState {
  /** The agent-provided session title, or `null` when unset/cleared. */
  title: string | null;
  /** ISO-8601 timestamp of last activity, or `null` when unset/cleared. */
  updatedAt: string | null;
}

/** Token and cost usage tracked from `usage_update`. */
export interface UsageState {
  /** Total context-window size, in tokens. */
  size: number;
  /** Tokens currently occupying the context window. */
  used: number;
  /** Cumulative session cost, when the agent reports it. */
  cost: { amount: number; currency: string } | null;
}

/** The initial, empty conversation store — a fresh stateless conversation. */
export const initialConversationState: ConversationState = {
  messages: [],
  availableCommands: [],
  currentModeId: null,
  configOptions: [],
  sessionInfo: { title: null, updatedAt: null },
  usage: null,
};

/**
 * Monotonic id source for synthesized messages.
 *
 * ACP message chunks carry an *optional, unstable* `messageId`. When present
 * it groups chunks; when absent this counter synthesizes a stable id so chunks
 * of the same streaming turn still coalesce. The counter is module-scoped so
 * ids stay unique across reducer calls.
 */
let messageSeq = 0;

/** Mint a unique synthetic message id. */
function nextMessageId(): string {
  messageSeq += 1;
  return `msg-${messageSeq}`;
}

/** The last element of an array, or `undefined` when the array is empty. */
function lastOf<T>(items: readonly T[]): T | undefined {
  return items.length > 0 ? items[items.length - 1] : undefined;
}

/**
 * Whether a message is finalized — its streaming text/reasoning is `done`.
 *
 * A message is finalized once every `text`/`reasoning` part it carries is in
 * `state: "done"`: either the turn ended (see {@link finalizeStreamingParts})
 * or the message was appended already-complete (see {@link appendUserPrompt}).
 * A finalized message belongs to a settled turn — a later streaming chunk must
 * never reopen it, so {@link coalesceChunk} refuses to grow one.
 *
 * A message with no `text`/`reasoning` parts at all (e.g. a tool-only message)
 * is *not* treated as finalized: it has no streaming part to protect and may
 * still legitimately gain a streaming text run.
 */
function isFinalized(message: ConversationMessage): boolean {
  let sawStreamingPart = false;
  for (const part of message.parts) {
    if (part.type !== "text" && part.type !== "reasoning") {
      continue;
    }
    sawStreamingPart = true;
    if (part.state !== "done") {
      return false;
    }
  }
  return sawStreamingPart;
}

/**
 * Coalesce a streaming text/reasoning chunk into the last message.
 *
 * Streaming chunks of one turn must grow a *single* part rather than appending
 * a new part per chunk. A chunk coalesces into the last message when:
 *
 * - that message has the expected `role`, and
 * - either the chunk's `messageId` matches the message's id, or the chunk
 *   carries no `messageId` and the message was the most recent of that role,
 *   and
 * - that message is not already finalized — a `done` message belongs to a
 *   completed turn and a chunk must never reopen it (see {@link isFinalized}).
 *
 * Otherwise a fresh message is started. The grown part is always the *last*
 * part of the matching kind, so interleaved tool parts do not break coalescing
 * of a later text run — a new run simply appends a new part.
 *
 * @param messages - The current message log.
 * @param role - The role the chunk belongs to (`user` or `assistant`).
 * @param partType - Which streaming part kind to grow (`text` or `reasoning`).
 * @param text - The chunk's text to append.
 * @param messageId - The chunk's ACP `messageId`, when the agent supplied one.
 * @returns A new message log with the chunk folded in.
 */
function coalesceChunk(
  messages: ConversationMessage[],
  role: "user" | "assistant",
  partType: "text" | "reasoning",
  text: string,
  messageId: string | null | undefined,
): ConversationMessage[] {
  const last = lastOf(messages);
  const canExtend =
    last !== undefined &&
    last.role === role &&
    (messageId == null || last.id === messageId) &&
    !isFinalized(last);

  if (!canExtend) {
    const fresh: ConversationMessage = {
      id: messageId ?? nextMessageId(),
      role,
      parts: [makeStreamingPart(partType, text)],
    };
    return [...messages, fresh];
  }

  const grown = growLastPart(last.parts, partType, text);
  const updated: ConversationMessage = { ...last, parts: grown };
  return [...messages.slice(0, -1), updated];
}

/** Build a fresh streaming `text` or `reasoning` part holding `text`. */
function makeStreamingPart(
  partType: "text" | "reasoning",
  text: string,
): TextUIPart | ReasoningUIPart {
  return { type: partType, text, state: "streaming" };
}

/**
 * Append `text` to the last `partType` part of `parts`, or add a new one.
 *
 * Returns a new parts array; the input is never mutated. When the last part of
 * the requested kind is not the final element (a tool part was emitted after
 * it), a new part is appended so the two text runs render in order.
 */
function growLastPart(
  parts: ConversationPart[],
  partType: "text" | "reasoning",
  text: string,
): ConversationPart[] {
  const lastIndex = parts.length - 1;
  const lastPart = parts[lastIndex];

  if (lastPart !== undefined && lastPart.type === partType) {
    const grown = {
      ...lastPart,
      text: lastPart.text + text,
      state: "streaming" as const,
    };
    return [...parts.slice(0, lastIndex), grown];
  }

  return [...parts, makeStreamingPart(partType, text)];
}

/**
 * Extract the plain text of an ACP {@link ContentBlock}.
 *
 * Message chunks carry a `ContentBlock`. AI Elements renders text, so a
 * `text` block contributes its `text`; an `image`/`audio` block contributes a
 * short bracketed placeholder; a `resource_link` contributes its name or URI;
 * an embedded `resource` contributes its embedded text when present. This
 * keeps every block kind renderable without inventing parts AI Elements has no
 * component for.
 */
function contentBlockText(block: ContentBlock): string {
  switch (block.type) {
    case "text":
      return block.text;
    case "image":
      return "[image]";
    case "audio":
      return "[audio]";
    case "resource_link":
      return block.name
        ? `[resource: ${block.name}]`
        : `[resource: ${block.uri}]`;
    case "resource": {
      const resource = block.resource;
      return "text" in resource && typeof resource.text === "string"
        ? resource.text
        : `[resource: ${resource.uri}]`;
    }
    default:
      return "";
  }
}

/**
 * The subset of `DynamicToolUIPart` states this adapter emits.
 *
 * ACP's `ToolCallStatus` has four values and none of them carry an approval
 * decision, so the adapter only ever produces these four AI SDK states — never
 * the approval-flavored ones (`approval-requested`, `output-denied`, …). ACP
 * permission requests are surfaced through {@link ConversationApi.permissionRequest}
 * instead, not through tool-part approval states.
 */
type AdapterToolState =
  | "input-streaming"
  | "input-available"
  | "output-available"
  | "output-error";

/**
 * Map an ACP {@link ToolCallStatus} to an AI SDK tool-part `state`.
 *
 * Covers exactly the four statuses ACP emits. A missing status is treated as
 * `pending`, the status ACP documents as the default for a freshly created
 * tool call.
 */
function toolStateFor(
  status: ToolCallStatus | null | undefined,
): AdapterToolState {
  switch (status) {
    case "in_progress":
      return "input-available";
    case "completed":
      return "output-available";
    case "failed":
      return "output-error";
    default:
      return "input-streaming";
  }
}

/**
 * Project the content of an ACP tool call into a renderable output value.
 *
 * Prefers the structured `rawOutput` when the agent supplied one; otherwise
 * falls back to the concatenated text of every `content` block. Returns
 * `undefined` when the tool produced nothing yet.
 */
function toolOutputFor(
  rawOutput: unknown,
  content: ToolCallContent[] | null | undefined,
): unknown {
  if (rawOutput !== undefined && rawOutput !== null) {
    return rawOutput;
  }
  if (!content || content.length === 0) {
    return undefined;
  }
  const text = content
    .filter(
      (entry): entry is ToolCallContent & { type: "content" } =>
        entry.type === "content",
    )
    .map((entry) => contentBlockText(entry.content))
    .join("");
  return text.length > 0 ? text : undefined;
}

/**
 * Build a {@link DynamicToolUIPart} from an ACP tool-call snapshot.
 *
 * AI SDK tool parts are a discriminated union keyed on `state`, so this builds
 * the correct member for the call's status: `output-error` carries `errorText`
 * and no `output`; `output-available` carries `output`; the input states
 * carry only `input`. `name`/`kind` ride along as `toolName` and `title`.
 */
function buildToolPart(snapshot: ToolCallSnapshot): DynamicToolUIPart {
  const base = {
    type: "dynamic-tool" as const,
    toolName: snapshot.toolName,
    toolCallId: snapshot.toolCallId,
    title: snapshot.title,
    toolMetadata: snapshot.kind ? { kind: snapshot.kind } : undefined,
  };
  const state = toolStateFor(snapshot.status);

  if (state === "output-error") {
    return {
      ...base,
      state,
      input: snapshot.input,
      errorText: snapshot.errorText ?? "the tool call failed",
    };
  }
  if (state === "output-available") {
    return {
      ...base,
      state,
      input: snapshot.input,
      output: snapshot.output,
    };
  }
  return { ...base, state, input: snapshot.input };
}

/**
 * The accumulated, mutation-free view of one ACP tool call.
 *
 * ACP delivers a tool call as an initial `tool_call` plus any number of
 * partial `tool_call_update`s — only changed fields are present on an update.
 * This snapshot is the merged result, the single input {@link buildToolPart}
 * needs to emit the current `DynamicToolUIPart`.
 */
interface ToolCallSnapshot {
  /** The tool call's session-unique id. */
  toolCallId: string;
  /** The tool's name — its ACP `title`, the agent's human-readable label. */
  toolName: string;
  /** The human-readable title shown in the tool header. */
  title: string;
  /** The ACP tool kind, when reported. */
  kind: ToolKind | null;
  /** The latest reported status. */
  status: ToolCallStatus | null;
  /** The raw tool input parameters. */
  input: unknown;
  /** The raw tool output, or text projected from content blocks. */
  output: unknown;
  /** The failure message, when the call failed. */
  errorText: string | null;
}

/**
 * Locate a `dynamic-tool` part by its `toolCallId` across the message log.
 *
 * Returns the owning message index, the part index within that message, and
 * the part itself — or `null` when no such tool part exists yet (the agent
 * sent a `tool_call_update` for a call it never opened).
 */
function findToolPart(
  messages: ConversationMessage[],
  toolCallId: string,
): { messageIndex: number; partIndex: number; part: DynamicToolUIPart } | null {
  for (let m = 0; m < messages.length; m += 1) {
    const parts = messages[m].parts;
    for (let p = 0; p < parts.length; p += 1) {
      const part = parts[p];
      if (part.type === "dynamic-tool" && part.toolCallId === toolCallId) {
        return { messageIndex: m, partIndex: p, part };
      }
    }
  }
  return null;
}

/**
 * Map an AI SDK tool-part `state` back to an ACP {@link ToolCallStatus}.
 *
 * The inverse of {@link toolStateFor}: it recovers the ACP status from a
 * rendered {@link DynamicToolUIPart} so a `tool_call_update` can be merged onto
 * the prior call state. Only the four {@link AdapterToolState} values this
 * adapter ever emits need a case; the parameter type guarantees exhaustiveness.
 */
function statusForToolState(state: AdapterToolState): ToolCallStatus {
  switch (state) {
    case "output-available":
      return "completed";
    case "output-error":
      return "failed";
    case "input-available":
      return "in_progress";
    case "input-streaming":
      return "pending";
  }
}

/**
 * Reconstruct a {@link ToolCallSnapshot} from an existing rendered tool part.
 *
 * A `tool_call_update` carries only changed fields, so updating a tool part
 * means merging the delta onto the call's prior state. The prior state is
 * recovered from the rendered `DynamicToolUIPart` — the adapter's single
 * source of truth — so no parallel bookkeeping is needed.
 */
function snapshotFromPart(part: DynamicToolUIPart): ToolCallSnapshot {
  const status = statusForToolState(part.state as AdapterToolState);
  const kind = part.toolMetadata?.kind;
  return {
    toolCallId: part.toolCallId,
    toolName: part.toolName,
    title: part.title ?? part.toolName,
    kind: typeof kind === "string" ? (kind as ToolKind) : null,
    status,
    input: part.input,
    output: part.state === "output-available" ? part.output : undefined,
    errorText: part.state === "output-error" ? part.errorText : null,
  };
}

/**
 * Append a brand-new tool call to the log as a `dynamic-tool` part.
 *
 * A `tool_call` always belongs to the assistant. It coalesces onto the last
 * assistant message — keeping a tool call visually attached to the text that
 * announced it — and otherwise starts a fresh assistant message.
 */
function appendToolCall(
  messages: ConversationMessage[],
  update: Extract<SessionUpdate, { sessionUpdate: "tool_call" }>,
): ConversationMessage[] {
  const snapshot: ToolCallSnapshot = {
    toolCallId: update.toolCallId,
    toolName: update.title,
    title: update.title,
    kind: update.kind ?? null,
    status: update.status ?? null,
    input: update.rawInput,
    output: toolOutputFor(update.rawOutput, update.content),
    errorText: null,
  };
  const part = buildToolPart(snapshot);
  const last = lastOf(messages);

  if (last !== undefined && last.role === "assistant") {
    const updated: ConversationMessage = {
      ...last,
      parts: [...last.parts, part],
    };
    return [...messages.slice(0, -1), updated];
  }

  const fresh: ConversationMessage = {
    id: nextMessageId(),
    role: "assistant",
    parts: [part],
  };
  return [...messages, fresh];
}

/**
 * Merge a `tool_call_update` into the matching `dynamic-tool` part.
 *
 * Only the fields the update carries override the prior snapshot; the rest are
 * preserved. An update for an unknown `toolCallId` is dropped — there is no
 * tool part to grow and inventing one would misrepresent the stream.
 */
function applyToolUpdate(
  messages: ConversationMessage[],
  update: Extract<SessionUpdate, { sessionUpdate: "tool_call_update" }>,
): ConversationMessage[] {
  const found = findToolPart(messages, update.toolCallId);
  if (found === null) {
    return messages;
  }

  const prior = snapshotFromPart(found.part);
  const merged: ToolCallSnapshot = {
    ...prior,
    title: update.title ?? prior.title,
    toolName: update.title ?? prior.toolName,
    kind: update.kind ?? prior.kind,
    status: update.status ?? prior.status,
    input: update.rawInput !== undefined ? update.rawInput : prior.input,
    output:
      update.rawOutput !== undefined || update.content !== undefined
        ? toolOutputFor(update.rawOutput, update.content)
        : prior.output,
    // ACP's `tool_call_update` carries no error-message field — a failure is
    // signalled by `status: "failed"` alone — so `errorText` can only be the
    // prior snapshot's. `buildToolPart` supplies a default message when a
    // failed call still has none.
    errorText: prior.errorText,
  };
  const part = buildToolPart(merged);

  const messageParts = messages[found.messageIndex].parts;
  const nextParts = [
    ...messageParts.slice(0, found.partIndex),
    part,
    ...messageParts.slice(found.partIndex + 1),
  ];
  const nextMessage: ConversationMessage = {
    ...messages[found.messageIndex],
    parts: nextParts,
  };
  return [
    ...messages.slice(0, found.messageIndex),
    nextMessage,
    ...messages.slice(found.messageIndex + 1),
  ];
}

/**
 * Fold a `plan` update into the message log as a single `data-plan` part.
 *
 * ACP plans are *replace-on-update*: every `plan` notification carries the
 * complete entry list. So this adapter keeps exactly one `data-plan` part —
 * the latest plan replaces the previous one in place rather than appending a
 * second plan part.
 */
function applyPlan(
  messages: ConversationMessage[],
  update: Extract<SessionUpdate, { sessionUpdate: "plan" }>,
): ConversationMessage[] {
  const planPart: ConversationPart = {
    type: "data-plan",
    data: { entries: update.entries },
  };

  for (let m = 0; m < messages.length; m += 1) {
    const parts = messages[m].parts;
    const planIndex = parts.findIndex((part) => part.type === "data-plan");
    if (planIndex !== -1) {
      const nextParts = [
        ...parts.slice(0, planIndex),
        planPart,
        ...parts.slice(planIndex + 1),
      ];
      const nextMessage: ConversationMessage = {
        ...messages[m],
        parts: nextParts,
      };
      return [...messages.slice(0, m), nextMessage, ...messages.slice(m + 1)];
    }
  }

  const last = lastOf(messages);
  if (last !== undefined && last.role === "assistant") {
    const updated: ConversationMessage = {
      ...last,
      parts: [...last.parts, planPart],
    };
    return [...messages.slice(0, -1), updated];
  }
  const fresh: ConversationMessage = {
    id: nextMessageId(),
    role: "assistant",
    parts: [planPart],
  };
  return [...messages, fresh];
}

/**
 * Fold a `user_message_chunk` into the message log.
 *
 * Some ACP agents *echo* the user's prompt back as `user_message_chunk`
 * notifications. The client has already appended that prompt locally as a
 * finalized user message the instant it was sent (see {@link appendUserPrompt}),
 * so an echo is redundant: folding it in would either grow — and reopen — that
 * finalized message (no `messageId`) or append a second, duplicate user
 * message (a distinct `messageId`).
 *
 * So when the most recent message is a finalized user message, a
 * `user_message_chunk` is treated as an echo and dropped. Otherwise it is a
 * genuine streaming user message (an agent that streams user input before any
 * local prompt) and {@link coalesceChunk} folds it normally.
 */
function foldUserChunk(
  messages: ConversationMessage[],
  text: string,
  messageId: string | null | undefined,
): ConversationMessage[] {
  const last = lastOf(messages);
  if (last !== undefined && last.role === "user" && isFinalized(last)) {
    return messages;
  }
  return coalesceChunk(messages, "user", "text", text, messageId);
}

/**
 * The pure conversation reducer: fold one ACP `session/update` into the store.
 *
 * Exhaustive over the {@link SessionUpdate} discriminated union — every variant
 * the installed `@agentclientprotocol/sdk` defines has an explicit arm. This
 * function has no React dependency, which is what makes the per-variant
 * translation directly unit-testable.
 *
 * @param state - The current conversation store.
 * @param notification - One ACP `session/update` notification.
 * @returns The next conversation store. The input is never mutated.
 */
export function applySessionUpdate(
  state: ConversationState,
  notification: SessionNotification,
): ConversationState {
  const update = notification.update;
  switch (update.sessionUpdate) {
    case "user_message_chunk":
      return {
        ...state,
        messages: foldUserChunk(
          state.messages,
          contentBlockText(update.content),
          update.messageId,
        ),
      };
    case "agent_message_chunk":
      return {
        ...state,
        messages: coalesceChunk(
          state.messages,
          "assistant",
          "text",
          contentBlockText(update.content),
          update.messageId,
        ),
      };
    case "agent_thought_chunk":
      return {
        ...state,
        messages: coalesceChunk(
          state.messages,
          "assistant",
          "reasoning",
          contentBlockText(update.content),
          update.messageId,
        ),
      };
    case "tool_call":
      return { ...state, messages: appendToolCall(state.messages, update) };
    case "tool_call_update":
      return { ...state, messages: applyToolUpdate(state.messages, update) };
    case "plan":
      return { ...state, messages: applyPlan(state.messages, update) };
    case "available_commands_update":
      return { ...state, availableCommands: update.availableCommands };
    case "current_mode_update":
      return { ...state, currentModeId: update.currentModeId };
    case "config_option_update":
      return { ...state, configOptions: update.configOptions };
    case "session_info_update":
      return {
        ...state,
        sessionInfo: {
          title: update.title ?? null,
          updatedAt: update.updatedAt ?? null,
        },
      };
    case "usage_update":
      return {
        ...state,
        usage: {
          size: update.size,
          used: update.used,
          cost: update.cost
            ? { amount: update.cost.amount, currency: update.cost.currency }
            : null,
        },
      };
    default:
      // Exhaustive: every SessionUpdate variant is handled above. An unknown
      // variant from a future SDK is ignored rather than corrupting the store.
      return state;
  }
}

/**
 * Append a user prompt's content blocks to the log as a user message.
 *
 * Appending the prompt locally on send guarantees the user's own message is
 * visible the instant they submit it. The message is appended already
 * finalized (`state: "done"`) — the user's text is complete on send and never
 * streams in.
 *
 * Some ACP agents *also* echo the prompt back as `user_message_chunk`
 * notifications. That echo is redundant given this local append, so
 * {@link foldUserChunk} drops a `user_message_chunk` whenever the latest
 * message is a finalized user message — the finalized state set here is the
 * exact signal that distinguishes a settled local prompt from a genuine
 * streaming user message. Each `sendPrompt` starts a distinct user message.
 */
function appendUserPrompt(
  state: ConversationState,
  prompt: ContentBlock[],
): ConversationState {
  const text = prompt.map(contentBlockText).join("");
  const message: ConversationMessage = {
    id: nextMessageId(),
    role: "user",
    parts: [{ type: "text", text, state: "done" }],
  };
  return { ...state, messages: [...state.messages, message] };
}

/**
 * Mark every streaming `text`/`reasoning` part of the log as `done`.
 *
 * A turn ends when the agent reports a stop reason. At that point no more
 * chunks will arrive, so the AI Elements components should stop showing the
 * streaming affordance (the thinking shimmer, the streaming cursor).
 */
function finalizeStreamingParts(
  messages: ConversationMessage[],
): ConversationMessage[] {
  return messages.map((message) => {
    const hasStreaming = message.parts.some(
      (part) =>
        (part.type === "text" || part.type === "reasoning") &&
        part.state === "streaming",
    );
    if (!hasStreaming) {
      return message;
    }
    const parts = message.parts.map((part) =>
      (part.type === "text" || part.type === "reasoning") &&
      part.state === "streaming"
        ? { ...part, state: "done" as const }
        : part,
    );
    return { ...message, parts };
  });
}

/**
 * The internal reducer action set for {@link useConversation}'s store.
 *
 * `update` folds an ACP notification; `prompt-sent`/`turn-ended`/`turn-failed`
 * drive the turn lifecycle; `reset` clears the store for a new conversation.
 */
type ConversationAction =
  | { kind: "update"; notification: SessionNotification }
  | { kind: "prompt-sent"; prompt: ContentBlock[] }
  | { kind: "turn-ended"; stopReason: StopReason }
  | { kind: "turn-failed" }
  | { kind: "reset" };

/** The reducer state: the conversation store plus the live turn status. */
interface ConversationReducerState {
  /** The folded conversation store. */
  conversation: ConversationState;
  /** The turn-status state machine value. */
  status: ConversationStatus;
}

/** The initial reducer state — an empty, idle conversation. */
const initialReducerState: ConversationReducerState = {
  conversation: initialConversationState,
  status: "idle",
};

/**
 * The turn-aware conversation reducer.
 *
 * Layers turn-status transitions on top of {@link applySessionUpdate}:
 *
 * - `prompt-sent` — appends the user's message and enters `streaming`.
 * - `update` — folds an ACP notification (status unchanged).
 * - `turn-ended` — finalizes streaming parts; `refusal` lands in `error`,
 *   every other stop reason lands in `idle`.
 * - `turn-failed` — a transport/protocol failure; finalizes parts, `error`.
 * - `reset` — clears the store back to {@link initialReducerState}.
 */
export function conversationReducer(
  state: ConversationReducerState,
  action: ConversationAction,
): ConversationReducerState {
  switch (action.kind) {
    case "prompt-sent":
      return {
        conversation: appendUserPrompt(state.conversation, action.prompt),
        status: "streaming",
      };
    case "update":
      return {
        ...state,
        conversation: applySessionUpdate(
          state.conversation,
          action.notification,
        ),
      };
    case "turn-ended":
      return {
        conversation: {
          ...state.conversation,
          messages: finalizeStreamingParts(state.conversation.messages),
        },
        status: action.stopReason === "refusal" ? "error" : "idle",
      };
    case "turn-failed":
      return {
        conversation: {
          ...state.conversation,
          messages: finalizeStreamingParts(state.conversation.messages),
        },
        status: "error",
      };
    case "reset":
      return initialReducerState;
    default:
      return state;
  }
}

/**
 * A pending permission request awaiting the user's decision.
 *
 * When the agent calls `session/request_permission`, the hook stores the
 * request here and the panel renders the options. {@link ConversationApi.respondPermission}
 * resolves it. `null` means no request is pending.
 */
export type PermissionRequestState = RequestPermissionRequest | null;

/**
 * A pending elicitation request awaiting the user's structured input.
 *
 * When the agent calls `unstable_createElicitation`, the hook stores the request
 * here and the panel renders the form (or the url link). {@link ConversationApi.respondElicitation}
 * resolves it. `null` means no elicitation is pending. The agent may also
 * dismiss it out-of-band via `unstable_completeElicitation`, which clears this
 * back to `null` without a user response.
 */
export type ElicitationRequestState = CreateElicitationRequest | null;

/**
 * The hook surface the AI panel consumes.
 *
 * This is exactly the surface the task specifies — no more, no less.
 */
export interface ConversationApi {
  /** The message log, directly renderable by AI Elements as `UIMessage[]`. */
  messages: ConversationMessage[];
  /** The turn status: `idle`, `streaming`, or `error`. */
  status: ConversationStatus;
  /** The full conversation store, for panels that show session metadata. */
  state: ConversationState;
  /**
   * Send a user prompt and run a turn.
   *
   * Appends the user's message, enters `streaming`, starts the session lazily
   * on first send, and drives `prompt` to its stop reason.
   *
   * @param prompt - The user message content blocks.
   */
  sendPrompt(prompt: ContentBlock[]): Promise<void>;
  /**
   * Eagerly start the session without sending a prompt.
   *
   * Fire-and-forget: connects the client and opens the `newSession` so the
   * agent's session-start notifications — notably the
   * `available_commands_update` that drives the composer's `/` slash-command
   * menu — arrive before the user's first message. Idempotent and safe to race
   * with {@link sendPrompt}: both share one in-flight session, so the agent is
   * started exactly once. A warm-up failure is swallowed (the next
   * {@link sendPrompt} retries and surfaces errors through the turn status).
   */
  warmUp(): void;
  /** Cancel the in-flight prompt turn, if any. */
  cancel(): Promise<void>;
  /**
   * Reset the store and start a fresh, stateless conversation.
   *
   * Clears every message and ambient field, then drops the current session so
   * the next {@link sendPrompt} opens a brand-new `newSession`.
   */
  newConversation(): void;
  /** The pending permission request, or `null` when none is awaiting. */
  permissionRequest: PermissionRequestState;
  /**
   * Resolve the pending permission request with the user's decision.
   *
   * @param response - The user's permission outcome to return to the agent.
   */
  respondPermission(response: RequestPermissionResponse): void;
  /** The pending elicitation request, or `null` when none is awaiting. */
  elicitationRequest: ElicitationRequestState;
  /**
   * Resolve the pending elicitation request with the user's input.
   *
   * @param response - The user's elicitation outcome to return to the agent —
   *   an `accept` with content, or a `decline`/`cancel`.
   */
  respondElicitation(response: CreateElicitationResponse): void;
}

/**
 * The pair of agent->client handlers `createKanbanClient` needs.
 *
 * `useConversation` owns these handlers — they fold the `sessionUpdate` stream
 * into the store and surface permission and elicitation requests to the panel —
 * and hands them to the {@link ConversationConnect} factory so the ACP client
 * routes its callbacks straight into the conversation store.
 */
export interface ConversationHandlers {
  /** Forwards each `session/update` notification into the conversation store. */
  onSessionUpdate: (notification: SessionNotification) => void;
  /** Surfaces an agent permission request and awaits the user's decision. */
  onRequestPermission: (
    request: RequestPermissionRequest,
  ) => Promise<RequestPermissionResponse>;
  /** Surfaces an agent elicitation request and awaits the user's input. */
  onElicitation: (
    request: CreateElicitationRequest,
  ) => Promise<CreateElicitationResponse>;
  /** Dismisses a pending elicitation the agent reports finished out-of-band. */
  onCompleteElicitation: (
    notification: CompleteElicitationNotification,
  ) => void;
}

/**
 * Factory that builds (or reuses) the connected ACP client.
 *
 * The panel owns the WebSocket and the `createKanbanClient` call, but the
 * client's `onSessionUpdate`/`onRequestPermission`/elicitation handlers belong
 * to the conversation hook. So the panel passes a factory:
 * {@link useConversation} supplies its handlers, and the factory builds a
 * {@link KanbanAcpClient} wired to them.
 *
 * The factory may return `null` before the agent has started — the hook then
 * stays inert until a later `sendPrompt` succeeds in connecting.
 *
 * @param handlers - The conversation hook's session-update, permission, and
 *   elicitation handlers, to pass through to `createKanbanClient`.
 * @returns The connected ACP client, or `null` when the agent is not ready.
 */
export type ConversationConnect = (
  handlers: ConversationHandlers,
) => Promise<KanbanAcpClient | null>;

/**
 * Dependencies injected into {@link useConversation}.
 *
 * The hook is given a {@link ConversationConnect} factory rather than a ready
 * client, because the client must be built *with* the hook's own handlers.
 */
export interface UseConversationOptions {
  /**
   * Builds the ACP client wired to the hook's handlers. Invoked lazily on the
   * first {@link ConversationApi.sendPrompt}, then again after each
   * {@link ConversationApi.newConversation}.
   */
  connect: ConversationConnect;
}

/**
 * React hook: a live conversation folded from one ACP client's update stream.
 *
 * The hook owns the conversation store and the turn-status machine, *is* the
 * source of the ACP client's `sessionUpdate`/`requestPermission`/elicitation
 * handlers, and exposes the {@link ConversationApi} the AI panel renders.
 *
 * The ACP client is built lazily through the injected {@link ConversationConnect}
 * factory so the client's handlers route straight into this hook's reducer.
 * The client and its first session are cached until {@link ConversationApi.newConversation}
 * drops them for a fresh, stateless `newSession`.
 *
 * @param options - The {@link ConversationConnect} factory.
 * @returns The {@link ConversationApi} for the AI panel.
 */
export function useConversation(
  options: UseConversationOptions,
): ConversationApi {
  const { connect } = options;
  const [state, dispatch] = useReducer(
    conversationReducer,
    initialReducerState,
  );
  const [permissionRequest, setPermissionRequest] =
    useState<PermissionRequestState>(null);
  const [elicitationRequest, setElicitationRequest] =
    useState<ElicitationRequestState>(null);

  /**
   * The connected ACP client, built lazily on first `sendPrompt`. A ref, not
   * state, because the client is plumbing the render never reads.
   */
  const clientRef = useRef<KanbanAcpClient | null>(null);

  /**
   * The live session, started lazily on first `sendPrompt`. A ref, not state,
   * because the session is plumbing the render never reads — only the message
   * log it produces.
   */
  const sessionRef = useRef<AcpSession | null>(null);

  /**
   * The in-flight {@link ensureSession} promise, cached so concurrent callers
   * (e.g. a warm-up effect racing the user's first send) share one session
   * start instead of each spawning an agent. Cleared once settled.
   */
  const sessionPromiseRef = useRef<Promise<AcpSession | null> | null>(null);

  /**
   * Bumped by {@link newConversation} to abandon any session start still in
   * flight. A start captures the generation at its outset and refuses to cache
   * its session if the generation has since changed — otherwise a new chat
   * begun mid-start would silently reuse the prior session.
   */
  const sessionGenerationRef = useRef(0);

  /**
   * Resolver for the in-flight permission request. The ACP client's
   * `onRequestPermission` handler returns this promise; `respondPermission`
   * resolves it. A ref so the handler and the responder share one slot.
   */
  const permissionResolverRef = useRef<
    ((response: RequestPermissionResponse) => void) | null
  >(null);

  /**
   * Resolver for the in-flight elicitation request. The ACP client's
   * `onElicitation` handler returns this promise; `respondElicitation` resolves
   * it. A ref so the handler and the responder share one slot.
   */
  const elicitationResolverRef = useRef<
    ((response: CreateElicitationResponse) => void) | null
  >(null);

  /**
   * Surface an agent permission request to the panel and await the decision.
   *
   * Passed to `createKanbanClient` as `onRequestPermission`. Stores the request
   * for the panel to render and returns a promise resolved by
   * {@link ConversationApi.respondPermission}.
   */
  const handleRequestPermission = useCallback(
    (request: RequestPermissionRequest): Promise<RequestPermissionResponse> => {
      setPermissionRequest(request);
      return new Promise<RequestPermissionResponse>((resolve) => {
        permissionResolverRef.current = resolve;
      });
    },
    [],
  );

  /**
   * Surface an agent elicitation request to the panel and await the input.
   *
   * Passed to `createKanbanClient` as `onElicitation`. Stores the request for
   * the panel to render and returns a promise resolved by
   * {@link ConversationApi.respondElicitation}.
   */
  const handleElicitation = useCallback(
    (request: CreateElicitationRequest): Promise<CreateElicitationResponse> => {
      console.info(
        "[elicitation] handleElicitation: surfacing request to panel",
        {
          mode: request.mode,
        },
      );
      setElicitationRequest(request);
      return new Promise<CreateElicitationResponse>((resolve) => {
        elicitationResolverRef.current = resolve;
      });
    },
    [],
  );

  /**
   * Dismiss a pending elicitation the agent reports finished out-of-band.
   *
   * Passed to `createKanbanClient` as `onCompleteElicitation`. A
   * `unstable_completeElicitation` notification means the agent resolved the
   * elicitation itself (typically a url-mode flow completing in the browser),
   * so the in-flight prompt is cleared. The resolver is dropped without being
   * called — the agent expects no client response to a completion.
   */
  const handleCompleteElicitation = useCallback(
    (_notification: CompleteElicitationNotification): void => {
      elicitationResolverRef.current = null;
      setElicitationRequest(null);
    },
    [],
  );

  /**
   * The handler set handed to the {@link ConversationConnect} factory.
   *
   * `dispatch` from `useReducer` is referentially stable, and the permission and
   * elicitation handlers are memoized, so this set is stable for the hook's
   * lifetime — the ACP client never needs rebuilding to refresh it.
   */
  const handlers = useMemo<ConversationHandlers>(
    () => ({
      onSessionUpdate: (notification) =>
        dispatch({ kind: "update", notification }),
      onRequestPermission: handleRequestPermission,
      onElicitation: handleElicitation,
      onCompleteElicitation: handleCompleteElicitation,
    }),
    [handleRequestPermission, handleElicitation, handleCompleteElicitation],
  );

  /**
   * Resolve the pending permission request.
   *
   * Clears the stored request and fulfils the promise the ACP client is
   * awaiting. A no-op when nothing is pending.
   */
  const respondPermission = useCallback(
    (response: RequestPermissionResponse) => {
      const resolve = permissionResolverRef.current;
      permissionResolverRef.current = null;
      setPermissionRequest(null);
      resolve?.(response);
    },
    [],
  );

  /**
   * Resolve the pending elicitation request.
   *
   * Clears the stored request and fulfils the promise the ACP client is
   * awaiting. A no-op when nothing is pending.
   */
  const respondElicitation = useCallback(
    (response: CreateElicitationResponse) => {
      const resolve = elicitationResolverRef.current;
      console.info(
        "[elicitation] respondElicitation: resolving pending request",
        {
          action: response.action,
          hasPendingResolver: resolve !== null,
        },
      );
      elicitationResolverRef.current = null;
      setElicitationRequest(null);
      resolve?.(response);
    },
    [],
  );

  /**
   * Lazily build the client and start its session, returning the session.
   *
   * The client is built once via {@link ConversationConnect} and the session
   * once via `startSession`; both are cached. Returns `null` when the factory
   * reports the agent is not ready.
   */
  const ensureSession = useCallback((): Promise<AcpSession | null> => {
    // A started session is reused directly.
    if (sessionRef.current !== null) {
      return Promise.resolve(sessionRef.current);
    }
    // A start already in flight is shared, so a warm-up racing the first send
    // does not spawn two agents.
    if (sessionPromiseRef.current !== null) {
      return sessionPromiseRef.current;
    }
    // Snapshot the generation so a `newConversation` mid-start can be detected.
    const generation = sessionGenerationRef.current;
    const start = (async (): Promise<AcpSession | null> => {
      if (clientRef.current === null) {
        clientRef.current = await connect(handlers);
      }
      const client = clientRef.current;
      if (client === null) {
        return null;
      }
      if (sessionRef.current === null) {
        const session = await client.startSession();
        // A `newConversation` that fired while `startSession` was in flight
        // bumped the generation, abandoning this start. Don't cache the
        // session — doing so would let the "fresh" chat silently reuse the
        // prior session. Return it un-cached so the next caller opens a new one.
        if (sessionGenerationRef.current !== generation) {
          return session;
        }
        sessionRef.current = session;
      }
      return sessionRef.current;
    })();
    sessionPromiseRef.current = start;
    // Drop the in-flight cache once settled so a failed attempt can be retried,
    // but only if this start still owns the slot — a newer start (after a
    // reset) must not have its in-flight promise cleared by an abandoned one.
    void start.finally(() => {
      if (sessionPromiseRef.current === start) {
        sessionPromiseRef.current = null;
      }
    });
    return start;
  }, [connect, handlers]);

  /**
   * Eagerly start the session without sending a prompt — see
   * {@link ConversationApi.warmUp}. Fire-and-forget; warm-up failures are
   * swallowed because the next {@link sendPrompt} retries and reports errors
   * through the turn status.
   */
  const warmUp = useCallback((): void => {
    void ensureSession().catch(() => {
      // Best-effort: a failed warm-up is not surfaced. The first real send
      // re-attempts and lands the turn in `error` if it still fails.
    });
  }, [ensureSession]);

  /**
   * Send a prompt and run the turn.
   *
   * Appends the user message and enters `streaming` before any await, so the
   * UI reflects the send immediately. A thrown error — transport failure or a
   * rejected `prompt` — lands the turn in `error`.
   */
  const sendPrompt = useCallback(
    async (prompt: ContentBlock[]): Promise<void> => {
      dispatch({ kind: "prompt-sent", prompt });
      try {
        const session = await ensureSession();
        if (session === null) {
          dispatch({ kind: "turn-failed" });
          return;
        }
        const response = await session.prompt(prompt);
        dispatch({ kind: "turn-ended", stopReason: response.stopReason });
      } catch {
        dispatch({ kind: "turn-failed" });
      }
    },
    [ensureSession],
  );

  /** Cancel the in-flight turn; a no-op when no session has been started. */
  const cancel = useCallback(async (): Promise<void> => {
    await sessionRef.current?.cancel();
  }, []);

  /**
   * Reset the store and drop the session so the next send starts fresh.
   *
   * The client connection is kept — it is reusable — but the session is
   * dropped, so the next {@link sendPrompt} opens a brand-new stateless
   * `newSession`.
   */
  const newConversation = useCallback(() => {
    sessionRef.current = null;
    // Drop any in-flight session start so the next send/warm-up opens a fresh
    // one rather than awaiting the abandoned session. Bumping the generation
    // also tells a start still resolving to discard its session rather than
    // caching it into the freshly reset conversation.
    sessionPromiseRef.current = null;
    sessionGenerationRef.current += 1;
    permissionResolverRef.current = null;
    setPermissionRequest(null);
    // An elicitation left pending when the conversation is reset is abandoned;
    // drop its resolver (the agent's session is gone) and clear the prompt.
    elicitationResolverRef.current = null;
    setElicitationRequest(null);
    dispatch({ kind: "reset" });
  }, []);

  return useMemo<ConversationApi>(
    () => ({
      messages: state.conversation.messages,
      status: state.status,
      state: state.conversation,
      sendPrompt,
      warmUp,
      cancel,
      newConversation,
      permissionRequest,
      respondPermission,
      elicitationRequest,
      respondElicitation,
    }),
    [
      state.conversation,
      state.status,
      sendPrompt,
      warmUp,
      cancel,
      newConversation,
      permissionRequest,
      respondPermission,
      elicitationRequest,
      respondElicitation,
    ],
  );
}
