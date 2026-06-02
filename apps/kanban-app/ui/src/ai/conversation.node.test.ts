/**
 * Translation tests for the ACP conversation adapter.
 *
 * `conversation.ts` folds the ACP `session/update` notification stream into
 * AI-SDK-`UIMessage`-shaped state that the AI Elements components render
 * directly. This file is the per-variant safety net for that fold:
 *
 * - {@link applySessionUpdate} — the pure reducer — is exercised once per
 *   `SessionUpdate` variant, asserting the resulting `UIMessage` part shape.
 * - Streaming text/thought chunk coalescing gets its own focused tests.
 * - {@link conversationReducer} — the turn-aware wrapper — is exercised for
 *   every turn-status transition (`idle` -> `streaming` -> `idle`/`error`).
 *
 * Node-only (no DOM, no React rendering) — the reducers are pure functions, so
 * they need no browser. Lives under the `*.node.test.ts` suffix recognized by
 * `vite.config.ts`.
 */
import { describe, it, expect } from "vitest";
import type {
  ContentBlock,
  SessionNotification,
  SessionUpdate,
} from "@agentclientprotocol/sdk";
import {
  applySessionUpdate,
  conversationReducer,
  initialConversationState,
  type ConversationMessage,
  type ConversationState,
} from "./conversation";

/** A fixed session id — the reducer never inspects it. */
const SESSION_ID = "session-test";

/** Wrap a {@link SessionUpdate} in the {@link SessionNotification} envelope. */
function notify(update: SessionUpdate): SessionNotification {
  return { sessionId: SESSION_ID, update };
}

/** A plain ACP text {@link ContentBlock}. */
function textBlock(text: string): ContentBlock {
  return { type: "text", text };
}

/** Fold a sequence of updates onto the empty store. */
function fold(...updates: SessionUpdate[]): ConversationState {
  return updates.reduce(
    (state, update) => applySessionUpdate(state, notify(update)),
    initialConversationState,
  );
}

/** Assert there is exactly one message and return it. */
function onlyMessage(state: ConversationState): ConversationMessage {
  expect(state.messages).toHaveLength(1);
  return state.messages[0];
}

describe("applySessionUpdate — per SessionUpdate variant", () => {
  it("user_message_chunk -> a user message with a text part", () => {
    const state = fold({
      sessionUpdate: "user_message_chunk",
      content: textBlock("hello agent"),
    });

    const message = onlyMessage(state);
    expect(message.role).toBe("user");
    expect(message.parts).toEqual([
      { type: "text", text: "hello agent", state: "streaming" },
    ]);
  });

  it("agent_message_chunk -> an assistant message with a text part", () => {
    const state = fold({
      sessionUpdate: "agent_message_chunk",
      content: textBlock("hello human"),
    });

    const message = onlyMessage(state);
    expect(message.role).toBe("assistant");
    expect(message.parts).toEqual([
      { type: "text", text: "hello human", state: "streaming" },
    ]);
  });

  it("agent_thought_chunk -> an assistant message with a reasoning part", () => {
    const state = fold({
      sessionUpdate: "agent_thought_chunk",
      content: textBlock("let me think"),
    });

    const message = onlyMessage(state);
    expect(message.role).toBe("assistant");
    expect(message.parts).toEqual([
      { type: "reasoning", text: "let me think", state: "streaming" },
    ]);
  });

  it("tool_call -> an assistant message with a dynamic-tool part", () => {
    const state = fold({
      sessionUpdate: "tool_call",
      toolCallId: "call-1",
      title: "search_board",
      kind: "search",
      status: "pending",
      rawInput: { query: "kanban" },
    });

    const message = onlyMessage(state);
    expect(message.role).toBe("assistant");
    expect(message.parts).toHaveLength(1);
    expect(message.parts[0]).toEqual({
      type: "dynamic-tool",
      toolName: "search_board",
      toolCallId: "call-1",
      title: "search_board",
      toolMetadata: { kind: "search" },
      state: "input-streaming",
      input: { query: "kanban" },
    });
  });

  it("tool_call_update -> updates the matching dynamic-tool part to completed", () => {
    const state = fold(
      {
        sessionUpdate: "tool_call",
        toolCallId: "call-1",
        title: "search_board",
        kind: "search",
        status: "in_progress",
        rawInput: { query: "kanban" },
      },
      {
        sessionUpdate: "tool_call_update",
        toolCallId: "call-1",
        status: "completed",
        rawOutput: { hits: 3 },
      },
    );

    const part = onlyMessage(state).parts[0];
    expect(part).toEqual({
      type: "dynamic-tool",
      toolName: "search_board",
      toolCallId: "call-1",
      title: "search_board",
      toolMetadata: { kind: "search" },
      state: "output-available",
      input: { query: "kanban" },
      output: { hits: 3 },
    });
  });

  it("tool_call_update with status failed -> output-error with errorText", () => {
    const state = fold(
      {
        sessionUpdate: "tool_call",
        toolCallId: "call-2",
        title: "fetch_url",
        status: "in_progress",
      },
      {
        sessionUpdate: "tool_call_update",
        toolCallId: "call-2",
        status: "failed",
        content: [{ type: "content", content: textBlock("request failed") }],
      },
    );

    const part = onlyMessage(state).parts[0];
    expect(part.type).toBe("dynamic-tool");
    if (part.type !== "dynamic-tool") throw new Error("expected dynamic-tool");
    expect(part.state).toBe("output-error");
    if (part.state !== "output-error") throw new Error("expected output-error");
    expect(part.errorText).toBe("the tool call failed");
  });

  it("tool_call_update for an unknown toolCallId is dropped", () => {
    const state = fold({
      sessionUpdate: "tool_call_update",
      toolCallId: "never-opened",
      status: "completed",
    });

    expect(state.messages).toEqual([]);
  });

  it("plan -> an assistant message with a data-plan part", () => {
    const state = fold({
      sessionUpdate: "plan",
      entries: [
        { content: "research", priority: "high", status: "in_progress" },
        { content: "implement", priority: "medium", status: "pending" },
      ],
    });

    const part = onlyMessage(state).parts[0];
    expect(part).toEqual({
      type: "data-plan",
      data: {
        entries: [
          { content: "research", priority: "high", status: "in_progress" },
          { content: "implement", priority: "medium", status: "pending" },
        ],
      },
    });
  });

  it("a second plan replaces the existing data-plan part in place", () => {
    const state = fold(
      {
        sessionUpdate: "plan",
        entries: [{ content: "step one", priority: "high", status: "pending" }],
      },
      {
        sessionUpdate: "plan",
        entries: [
          { content: "step one", priority: "high", status: "completed" },
          { content: "step two", priority: "low", status: "pending" },
        ],
      },
    );

    const message = onlyMessage(state);
    const planParts = message.parts.filter((p) => p.type === "data-plan");
    expect(planParts).toHaveLength(1);
    expect(planParts[0]).toEqual({
      type: "data-plan",
      data: {
        entries: [
          { content: "step one", priority: "high", status: "completed" },
          { content: "step two", priority: "low", status: "pending" },
        ],
      },
    });
  });

  it("available_commands_update -> the availableCommands state field", () => {
    const state = fold({
      sessionUpdate: "available_commands_update",
      availableCommands: [
        { name: "create_plan", description: "Draft an execution plan" },
      ],
    });

    expect(state.availableCommands).toEqual([
      { name: "create_plan", description: "Draft an execution plan" },
    ]);
    // An ambient session update produces no message.
    expect(state.messages).toEqual([]);
  });

  it("current_mode_update -> the currentModeId state field", () => {
    const state = fold({
      sessionUpdate: "current_mode_update",
      currentModeId: "code",
    });

    expect(state.currentModeId).toBe("code");
    expect(state.messages).toEqual([]);
  });

  it("config_option_update -> the configOptions state field", () => {
    const state = fold({
      sessionUpdate: "config_option_update",
      configOptions: [
        {
          type: "boolean",
          id: "verbose",
          name: "Verbose",
          description: null,
          currentValue: true,
        },
      ],
    });

    expect(state.configOptions).toHaveLength(1);
    expect(state.configOptions[0]).toMatchObject({
      id: "verbose",
      currentValue: true,
    });
    expect(state.messages).toEqual([]);
  });

  it("session_info_update -> the sessionInfo state field", () => {
    const state = fold({
      sessionUpdate: "session_info_update",
      title: "Refactor the board",
      updatedAt: "2026-05-18T12:00:00Z",
    });

    expect(state.sessionInfo).toEqual({
      title: "Refactor the board",
      updatedAt: "2026-05-18T12:00:00Z",
    });
    expect(state.messages).toEqual([]);
  });

  it("usage_update -> the usage state field", () => {
    const state = fold({
      sessionUpdate: "usage_update",
      size: 200_000,
      used: 12_500,
      cost: { amount: 0.42, currency: "USD" },
    });

    expect(state.usage).toEqual({
      size: 200_000,
      used: 12_500,
      cost: { amount: 0.42, currency: "USD" },
    });
    expect(state.messages).toEqual([]);
  });
});

describe("applySessionUpdate — streaming chunk coalescing", () => {
  it("coalesces successive agent_message_chunks into one growing text part", () => {
    const state = fold(
      { sessionUpdate: "agent_message_chunk", content: textBlock("Hel") },
      { sessionUpdate: "agent_message_chunk", content: textBlock("lo, ") },
      { sessionUpdate: "agent_message_chunk", content: textBlock("world") },
    );

    const message = onlyMessage(state);
    expect(message.parts).toEqual([
      { type: "text", text: "Hello, world", state: "streaming" },
    ]);
  });

  it("coalesces successive agent_thought_chunks into one growing reasoning part", () => {
    const state = fold(
      { sessionUpdate: "agent_thought_chunk", content: textBlock("first ") },
      { sessionUpdate: "agent_thought_chunk", content: textBlock("then ") },
      { sessionUpdate: "agent_thought_chunk", content: textBlock("done") },
    );

    const message = onlyMessage(state);
    expect(message.parts).toEqual([
      { type: "reasoning", text: "first then done", state: "streaming" },
    ]);
  });

  it("keeps text and reasoning as separate parts of one assistant message", () => {
    const state = fold(
      { sessionUpdate: "agent_thought_chunk", content: textBlock("thinking") },
      { sessionUpdate: "agent_message_chunk", content: textBlock("answer") },
    );

    const message = onlyMessage(state);
    // When the text chunk lands, the prior reasoning part transitions to
    // "done" — only the actively-growing part stays "streaming". See the
    // dedicated "close prior streaming parts" block below for the
    // motivation: AI Elements' Reasoning component animates the spinner
    // (and locks the toggle-to-read) while state === "streaming", so a
    // closed `<think>` block must surface as done immediately, not at
    // turn-ended.
    expect(message.parts).toEqual([
      { type: "reasoning", text: "thinking", state: "done" },
      { type: "text", text: "answer", state: "streaming" },
    ]);
  });

  it("a differing messageId starts a fresh message rather than coalescing", () => {
    const state = fold(
      {
        sessionUpdate: "agent_message_chunk",
        content: textBlock("turn one"),
        messageId: "00000000-0000-0000-0000-000000000001",
      },
      {
        sessionUpdate: "agent_message_chunk",
        content: textBlock("turn two"),
        messageId: "00000000-0000-0000-0000-000000000002",
      },
    );

    expect(state.messages).toHaveLength(2);
    expect(state.messages[0].id).toBe("00000000-0000-0000-0000-000000000001");
    expect(state.messages[1].id).toBe("00000000-0000-0000-0000-000000000002");
  });

  it("a matching messageId coalesces chunks into the same message", () => {
    const state = fold(
      {
        sessionUpdate: "agent_message_chunk",
        content: textBlock("one "),
        messageId: "00000000-0000-0000-0000-0000000000aa",
      },
      {
        sessionUpdate: "agent_message_chunk",
        content: textBlock("message"),
        messageId: "00000000-0000-0000-0000-0000000000aa",
      },
    );

    const message = onlyMessage(state);
    expect(message.parts).toEqual([
      { type: "text", text: "one message", state: "streaming" },
    ]);
  });

  it("an echoed user_message_chunk does not reopen a finalized user message", () => {
    // The client appends the prompt locally as a finalized user message; some
    // ACP agents then echo that prompt back as a `user_message_chunk`. The
    // echo (no messageId) must not grow — and reopen — the finalized message.
    const sent = conversationReducer(
      { conversation: initialConversationState, status: "idle" },
      { kind: "prompt-sent", prompt: [textBlock("hello agent")] },
    );

    const echoed = applySessionUpdate(sent.conversation, {
      sessionId: SESSION_ID,
      update: {
        sessionUpdate: "user_message_chunk",
        content: textBlock("hello agent"),
      },
    });

    const message = onlyMessage(echoed);
    expect(message.role).toBe("user");
    expect(message.parts).toEqual([
      { type: "text", text: "hello agent", state: "done" },
    ]);
  });

  it("an echoed user_message_chunk with a messageId is not duplicated", () => {
    // An echo carrying a distinct real messageId would otherwise start a
    // second, duplicate user message; it must still be dropped as redundant.
    const sent = conversationReducer(
      { conversation: initialConversationState, status: "idle" },
      { kind: "prompt-sent", prompt: [textBlock("ship it")] },
    );

    const echoed = applySessionUpdate(sent.conversation, {
      sessionId: SESSION_ID,
      update: {
        sessionUpdate: "user_message_chunk",
        content: textBlock("ship it"),
        messageId: "00000000-0000-0000-0000-0000000000ec",
      },
    });

    const message = onlyMessage(echoed);
    expect(message.role).toBe("user");
    expect(message.parts).toEqual([
      { type: "text", text: "ship it", state: "done" },
    ]);
  });

  it("a genuine streaming user_message_chunk before any local prompt coalesces", () => {
    // With no finalized local prompt, successive user chunks are a genuine
    // streaming user message and still coalesce into one growing text part.
    const state = fold(
      { sessionUpdate: "user_message_chunk", content: textBlock("typed ") },
      { sessionUpdate: "user_message_chunk", content: textBlock("by agent") },
    );

    const message = onlyMessage(state);
    expect(message.role).toBe("user");
    expect(message.parts).toEqual([
      { type: "text", text: "typed by agent", state: "streaming" },
    ]);
  });

  it("a tool call between text runs splits them into separate text parts", () => {
    const state = fold(
      { sessionUpdate: "agent_message_chunk", content: textBlock("before") },
      {
        sessionUpdate: "tool_call",
        toolCallId: "call-x",
        title: "do_thing",
        status: "pending",
      },
      { sessionUpdate: "agent_message_chunk", content: textBlock("after") },
    );

    const message = onlyMessage(state);
    expect(message.parts.map((p) => p.type)).toEqual([
      "text",
      "dynamic-tool",
      "text",
    ]);
    const [first, , third] = message.parts;
    expect(first).toMatchObject({ type: "text", text: "before" });
    expect(third).toMatchObject({ type: "text", text: "after" });
  });
});

describe("conversationReducer — turn-status transitions", () => {
  it("starts idle with an empty store", () => {
    const start = conversationReducer(undefined as never, { kind: "reset" });
    expect(start.status).toBe("idle");
    expect(start.conversation.messages).toEqual([]);
  });

  it("prompt-sent -> streaming, and appends the user's message", () => {
    const state = conversationReducer(
      { conversation: initialConversationState, status: "idle" },
      { kind: "prompt-sent", prompt: [textBlock("do the thing")] },
    );

    expect(state.status).toBe("streaming");
    expect(state.conversation.messages).toHaveLength(1);
    expect(state.conversation.messages[0]).toMatchObject({
      role: "user",
      parts: [{ type: "text", text: "do the thing", state: "done" }],
    });
  });

  it("prompt-sent then an echoed user_message_chunk yields one finalized user message", () => {
    // Regression: an ACP agent that echoes the prompt back must not produce a
    // doubled, never-finalized user message. Exactly one user message — still
    // finalized — must result from `prompt-sent` -> echoed `update`.
    const sent = conversationReducer(
      { conversation: initialConversationState, status: "idle" },
      { kind: "prompt-sent", prompt: [textBlock("do the thing")] },
    );
    const afterEcho = conversationReducer(sent, {
      kind: "update",
      notification: notify({
        sessionUpdate: "user_message_chunk",
        content: textBlock("do the thing"),
      }),
    });

    const userMessages = afterEcho.conversation.messages.filter(
      (m) => m.role === "user",
    );
    expect(userMessages).toHaveLength(1);
    expect(userMessages[0].parts).toEqual([
      { type: "text", text: "do the thing", state: "done" },
    ]);
    expect(afterEcho.status).toBe("streaming");
  });

  it("turn-ended with end_turn -> idle, and finalizes streaming parts", () => {
    const streaming = conversationReducer(
      { conversation: initialConversationState, status: "idle" },
      { kind: "prompt-sent", prompt: [textBlock("hi")] },
    );
    const withChunk = conversationReducer(streaming, {
      kind: "update",
      notification: notify({
        sessionUpdate: "agent_message_chunk",
        content: textBlock("partial"),
      }),
    });
    expect(withChunk.status).toBe("streaming");

    const ended = conversationReducer(withChunk, {
      kind: "turn-ended",
      stopReason: "end_turn",
    });

    expect(ended.status).toBe("idle");
    const assistant = ended.conversation.messages.find(
      (m) => m.role === "assistant",
    );
    expect(assistant?.parts[0]).toMatchObject({
      type: "text",
      state: "done",
    });
  });

  it("turn-ended with refusal -> error", () => {
    const ended = conversationReducer(
      { conversation: initialConversationState, status: "streaming" },
      { kind: "turn-ended", stopReason: "refusal" },
    );

    expect(ended.status).toBe("error");
  });

  it("turn-ended with cancelled -> idle", () => {
    const ended = conversationReducer(
      { conversation: initialConversationState, status: "streaming" },
      { kind: "turn-ended", stopReason: "cancelled" },
    );

    expect(ended.status).toBe("idle");
  });

  it("turn-failed -> error, and finalizes streaming parts", () => {
    const streaming = conversationReducer(
      { conversation: initialConversationState, status: "idle" },
      { kind: "prompt-sent", prompt: [textBlock("hi")] },
    );
    const withChunk = conversationReducer(streaming, {
      kind: "update",
      notification: notify({
        sessionUpdate: "agent_thought_chunk",
        content: textBlock("hmm"),
      }),
    });

    const failed = conversationReducer(withChunk, { kind: "turn-failed" });

    expect(failed.status).toBe("error");
    const assistant = failed.conversation.messages.find(
      (m) => m.role === "assistant",
    );
    expect(assistant?.parts[0]).toMatchObject({
      type: "reasoning",
      state: "done",
    });
  });

  it("reset clears the store and returns to idle", () => {
    const populated = conversationReducer(
      { conversation: initialConversationState, status: "idle" },
      { kind: "prompt-sent", prompt: [textBlock("something")] },
    );
    expect(populated.conversation.messages).not.toEqual([]);

    const cleared = conversationReducer(populated, { kind: "reset" });

    expect(cleared.status).toBe("idle");
    expect(cleared.conversation.messages).toEqual([]);
    expect(cleared.conversation).toEqual(initialConversationState);
  });

  it("update folds an ACP notification without changing the turn status", () => {
    const state = conversationReducer(
      { conversation: initialConversationState, status: "streaming" },
      {
        kind: "update",
        notification: notify({
          sessionUpdate: "current_mode_update",
          currentModeId: "ask",
        }),
      },
    );

    expect(state.status).toBe("streaming");
    expect(state.conversation.currentModeId).toBe("ask");
  });
});

/**
 * The "thinking…" spinner used to animate forever after `</think>` closed:
 * the prior reasoning part stayed in state:"streaming" until the entire
 * agentic loop ended (`turn-ended` → `finalizeStreamingParts`). The AI
 * Elements Reasoning component renders the spinner while
 * state === "streaming" and gates the toggle-to-read-think-text on
 * state === "done", so a stuck "streaming" state both kept the shimmer
 * and locked the toggle.
 *
 * The fix: every time a new chunk lands, finalize any prior streaming
 * REASONING part of the same message. Text parts are deliberately not
 * touched — they carry no visible streaming indicator and finalizing
 * them would falsely settle the message (see `isFinalized`) and block
 * subsequent chunks from coalescing.
 */
describe("applySessionUpdate — close prior streaming reasoning when a new chunk lands", () => {
  it("agent_message_chunk after agent_thought_chunk finalizes the reasoning part", () => {
    const state = fold(
      { sessionUpdate: "agent_thought_chunk", content: textBlock("plan") },
      { sessionUpdate: "agent_message_chunk", content: textBlock("answer") },
    );

    const message = onlyMessage(state);
    expect(message.parts).toEqual([
      { type: "reasoning", text: "plan", state: "done" },
      { type: "text", text: "answer", state: "streaming" },
    ]);
  });

  it("agent_thought_chunk after agent_message_chunk leaves the text part streaming", () => {
    // Text has no spinner, so finalizing it gains nothing user-visible AND
    // would falsely settle the message (`isFinalized` requires every
    // text/reasoning to be done). Keep it streaming.
    const state = fold(
      { sessionUpdate: "agent_message_chunk", content: textBlock("preface") },
      { sessionUpdate: "agent_thought_chunk", content: textBlock("more thinking") },
    );

    const message = onlyMessage(state);
    expect(message.parts).toEqual([
      { type: "text", text: "preface", state: "streaming" },
      { type: "reasoning", text: "more thinking", state: "streaming" },
    ]);
  });

  it("two consecutive agent_thought_chunks grow the same reasoning part (still streaming)", () => {
    const state = fold(
      { sessionUpdate: "agent_thought_chunk", content: textBlock("step 1") },
      { sessionUpdate: "agent_thought_chunk", content: textBlock(", step 2") },
    );

    const message = onlyMessage(state);
    expect(message.parts).toEqual([
      { type: "reasoning", text: "step 1, step 2", state: "streaming" },
    ]);
  });

  it("tool_call finalizes any prior streaming reasoning", () => {
    const state = fold(
      { sessionUpdate: "agent_thought_chunk", content: textBlock("decide") },
      { sessionUpdate: "agent_message_chunk", content: textBlock("calling tool") },
      {
        sessionUpdate: "tool_call",
        toolCallId: "call-1",
        title: "kanban",
        rawInput: { op: "list tasks" },
      },
    );

    const message = onlyMessage(state);
    // Reasoning is closed (spinner stops, toggle unlocks); the text part
    // stays streaming so it can keep coalescing more chunks if the model
    // resumes prose after the tool. The tool part is appended last.
    expect(message.parts[0]).toMatchObject({
      type: "reasoning",
      text: "decide",
      state: "done",
    });
    expect(message.parts[1]).toMatchObject({
      type: "text",
      text: "calling tool",
      state: "streaming",
    });
    expect(message.parts[2]).toMatchObject({ type: "dynamic-tool" });
  });

  it("interleaved think/visible/think/visible closes earlier reasoning, keeps text streaming", () => {
    const state = fold(
      { sessionUpdate: "agent_thought_chunk", content: textBlock("a") },
      { sessionUpdate: "agent_message_chunk", content: textBlock("X") },
      { sessionUpdate: "agent_thought_chunk", content: textBlock("b") },
      { sessionUpdate: "agent_message_chunk", content: textBlock("Y") },
    );

    const message = onlyMessage(state);
    expect(message.parts).toEqual([
      { type: "reasoning", text: "a", state: "done" },
      { type: "text", text: "X", state: "streaming" },
      { type: "reasoning", text: "b", state: "done" },
      { type: "text", text: "Y", state: "streaming" },
    ]);
  });
});
