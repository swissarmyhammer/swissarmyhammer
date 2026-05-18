---
assignees:
- claude-code
depends_on:
- 01KRRN4P0CN17AWCV2TQAF0NC1
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffff680
project: ai-panel
title: Conversation state from ACP sessionUpdate notifications
---
## What
The ACP `sessionUpdate` notification stream IS the conversation source of truth. Build the React hook that folds it into renderable message state — **this hook is what the AI Elements components consume.**

Do NOT adopt a chat framework (AI SDK `useChat`, TanStack AI). Those exist to talk to LLM providers and manage turns / streaming / tools — work the ACP agent and `ClientSideConnection` already do. Routing through one would mean two lossy adapters (ACP -> framework -> UIMessage), an extra (alpha) dependency, and a message model (`useChat` aside, TanStack's `ModelMessage`) that still is not AI Elements' `UIMessage`. Worse, a generic chat hook cannot express ACP's permission *requests*, plans, or session modes. This hook is the single, purpose-built adapter: ACP `SessionUpdate` -> AI-SDK-`UIMessage`-shaped parts that AI Elements renders directly.

- New `apps/kanban-app/ui/src/ai/conversation.ts` — a React hook / store consuming the ACP client's `sessionUpdate` callbacks.
- Translate each `SessionUpdate` variant into renderable message parts (AI SDK `UIMessage`-shaped, so AI Elements renders them directly):
  - agent message chunk -> assistant text part (streaming chunks coalesce)
  - agent thought chunk -> reasoning part
  - tool call / tool call update -> tool part (name, args, status, result)
  - plan -> plan/task part
  - available-commands changed -> available-commands state
- Track turn status (idle / streaming / error) from prompt start and the `prompt` stop reason.
- Expose the hook surface the panel needs: `{ messages, status, sendPrompt, cancel, newConversation, permissionRequest, respondPermission }`. "New conversation" resets the store and triggers a fresh `newSession` (stateless).

## Acceptance Criteria
- [x] Each `SessionUpdate` variant updates the conversation store to the documented `UIMessage` part shape.
- [x] Streaming text/thought chunks coalesce into a single growing part.
- [x] Turn status reflects streaming vs idle vs error; "new conversation" clears the store.
- [x] The hook exposes `messages` as `UIMessage[]` directly consumable by AI Elements — no chat-framework dependency.
- [x] `npm run build` in `apps/kanban-app/ui` succeeds.

## Tests
- [x] Vitest unit tests: feed each `SessionUpdate` variant, assert the resulting message state (one test per variant).
- [x] Test chunk coalescing and turn-status transitions.
- [x] `npm test` in `apps/kanban-app/ui` is green.

## Workflow
- Use `/tdd` — write the per-variant translation tests first.

## Implementation Notes

### Files
- `apps/kanban-app/ui/src/ai/conversation.ts` (new) — the purpose-built ACP→`UIMessage` adapter: the pure `applySessionUpdate` reducer, the turn-aware `conversationReducer`, and the `useConversation` React hook.
- `apps/kanban-app/ui/src/ai/conversation.node.test.ts` (new) — 32 pure-reducer tests: one per `SessionUpdate` variant, chunk-coalescing, turn-status transitions, and the user-prompt echo regression cases.
- `apps/kanban-app/ui/src/ai/conversation.test.tsx` (new) — 7 hook tests (browser project): `sendPrompt`/`cancel`/`newConversation`, error/refusal status, permission round trip, exact surface.

### `SessionUpdate` → `UIMessage` mapping
The installed `@agentclientprotocol/sdk` `SessionUpdate` union has **eleven** variants (more than the task's five). Every one has an explicit reducer arm:

| `SessionUpdate` variant       | Result                                                    |
|-------------------------------|-----------------------------------------------------------|
| `user_message_chunk`          | user message, `text` part (chunks coalesce; prompt echo dropped) |
| `agent_message_chunk`         | assistant message, `text` part (chunks coalesce)          |
| `agent_thought_chunk`         | assistant message, `reasoning` part (chunks coalesce)     |
| `tool_call`                   | assistant message, `dynamic-tool` part                    |
| `tool_call_update`            | merges the delta into the matching `dynamic-tool` part    |
| `plan`                        | single `data-plan` part (replace-on-update, in place)     |
| `available_commands_update`   | `ConversationState.availableCommands`                     |
| `current_mode_update`         | `ConversationState.currentModeId`                         |
| `config_option_update`        | `ConversationState.configOptions`                         |
| `session_info_update`         | `ConversationState.sessionInfo`                           |
| `usage_update`                | `ConversationState.usage`                                 |

### Key decisions
- **No chat framework.** Zero new dependencies. `messages` is typed `ConversationMessage[]`, which *is* `UIMessage<unknown, ConversationDataParts>[]` — AI Elements `Message`/`Tool`/`Reasoning`/`Task` consume it directly.
- **Tool parts use `dynamic-tool`.** ACP tool names are runtime-discovered, so the AI SDK `DynamicToolUIPart` (not a static `tool-${name}` part) is the correct shape. `ToolCallStatus` maps to four states — `pending`→`input-streaming`, `in_progress`→`input-available`, `completed`→`output-available`, `failed`→`output-error`. ACP permission requests are surfaced via `permissionRequest`, never via tool approval states.
- **Plan as a custom `data-plan` part.** AI Elements has no built-in plan part; ACP plans are replace-on-update, so exactly one `data-plan` part is kept and replaced in place. The panel renders it with the `Task` components.
- **Coalescing.** Streaming `text`/`reasoning` chunks grow the last matching part of one message. A differing ACP `messageId` (or a role change) starts a fresh message; an interleaved tool part splits a later text run into a new part so order is preserved. A chunk never grows a *finalized* (`state: "done"`) message — `coalesceChunk` checks `isFinalized` so a settled turn is never reopened.
- **User-prompt echo handling.** The client appends each prompt locally as a finalized (`state: "done"`) user message the instant it is sent. Some ACP agents *also* echo that prompt back as `user_message_chunk` notifications. `foldUserChunk` treats a `user_message_chunk` as a redundant echo — and drops it — whenever the latest message is a finalized user message. This is correct for *both* echo shapes: an echo with no `messageId` would otherwise grow and reopen the finalized message; an echo with a distinct real `messageId` would otherwise append a second, duplicate user message. A genuine streaming user message (an agent that streams user input before any local prompt) is *not* preceded by a finalized user message, so it still coalesces normally.
- **Turn status.** `prompt-sent`→`streaming`; `turn-ended` finalizes streaming parts (`state: "done"`) — `refusal` stop reason →`error`, every other reason →`idle`; transport/protocol failure →`error`; `reset` clears the store.
- **Pure reducer split.** `applySessionUpdate` and `conversationReducer` are React-free pure functions, which makes the per-variant translation exhaustively unit-testable in a `.node.test.ts` with no DOM.
- **Wiring.** The hook owns the ACP client's `onSessionUpdate`/`onRequestPermission` handlers, so it takes a `ConversationConnect` factory (panel passes `createKanbanClient` partially applied) rather than a ready client. `newConversation` keeps the reusable connection but drops the session for a fresh stateless `newSession`.

### Verification
- `npm run build` (`tsc && vite build`): success, 3763 modules.
- `npm test` (`tsc --noEmit && vitest run`): 2215 passed, 39 conversation tests green (`conversation.node.test.ts` 32, `conversation.test.tsx` 7), 0 type errors. The 3 failures are all known pre-existing and unrelated: 2 stale-fixture `slugify.parity` tests (task `01KRS426Q36ZN3DYBX2S0AS82T` — missing `slug_parity_corpus.txt`) and the CodeBlock/Shiki smoke flake (task `01KRVG4QSXPQ2FW5SG61M8EHAP`).

## Review Findings (2026-05-18 07:18)

Verified: all 11 SDK `SessionUpdate` variants are handled (confirmed against the installed `@agentclientprotocol/sdk` `types.gen.d.ts`); per-variant output shapes match the AI SDK `TextUIPart`/`ReasoningUIPart`/`DynamicToolUIPart`/`DataUIPart` definitions; chunk coalescing keys correctly on role + `messageId`; turn-status transitions are correct; the hook surface is exactly the documented seven members plus `state`; `package.json` adds no chat framework; `tsc --noEmit` is clean and all 35 new tests pass.

### Warnings
- [x] `apps/kanban-app/ui/src/ai/conversation.ts:739` — `appendUserPrompt` locally appends a `role: "user"` part with `state: "done"` on every `prompt-sent`. The module's own docstring (lines 734-735) notes some ACP agents *echo* the user prompt back as `user_message_chunk`. When such an agent echoes, `coalesceChunk("user", "text", …)` finds that locally-appended user message as the last message (matching role, no `messageId`), calls `growLastPart`, appends the echoed text onto the already-sent prompt, and flips its `state` back to `"streaming"` — producing a doubled, never-finalized user message. No test exercises the `prompt-sent` → `user_message_chunk` echo sequence. Suggested fix: skip coalescing into a finalized (`state: "done"`) user part, or have `appendUserPrompt` tag the synthetic message so an echo with a real `messageId` does not merge into it; add a regression test for the echo case.
  - RESOLVED: Two-part fix. (1) Added an `isFinalized(message)` helper and made `coalesceChunk` refuse to grow a finalized message — a chunk can never flip a `done` part back to `streaming`. (2) Added `foldUserChunk`, which the `user_message_chunk` reducer arm now uses: when the latest message is a finalized user message, the chunk is treated as a redundant echo of the locally-appended prompt and dropped. This handles both echo shapes — the no-`messageId` echo (would have reopened the finalized message) and the distinct-real-`messageId` echo (would have appended a duplicate message). A genuine streaming user message is never preceded by a finalized user message, so it still coalesces. Decision: the locally-appended prompt is the authoritative user message; the echo is dropped rather than merged, because merging cannot reliably de-duplicate text across the two echo shapes. Added 4 regression tests: echo-without-messageId, echo-with-messageId, genuine-streaming-user-chunk-still-coalesces, and the full `prompt-sent` → echoed-`update` turn sequence asserting exactly one finalized user message.

### Nits
- [x] `apps/kanban-app/ui/src/ai/conversation.ts:567` — In `applyToolUpdate`, `errorText: update.status === "failed" ? prior.errorText : prior.errorText` — both ternary arms are identical, so the condition is dead code. Replace with the plain `errorText: prior.errorText`, or implement the intended behavior if a `failed` status was meant to do something different.
  - RESOLVED: Verified against the installed SDK that `ToolCallUpdate` has no error-message field — ACP signals a failure with `status: "failed"` alone. So `errorText` can only come from the prior snapshot. Collapsed to `errorText: prior.errorText` with a comment noting the SDK shape and that `buildToolPart` supplies a default message for a failed call that still has none.
- [x] `apps/kanban-app/ui/src/ai/conversation.ts:478` — `snapshotFromPart` derives `status` via a 3-level chained ternary, which the JS/TS guidelines disallow ("No nested ternaries"). `buildToolPart` already does the inverse mapping with a clean `switch` in `toolStateFor`; mirror that with a small `switch`-based helper for the part-state→`ToolCallStatus` direction.
  - RESOLVED: Added `statusForToolState(state: AdapterToolState): ToolCallStatus` — a `switch` helper mirroring `toolStateFor`. `snapshotFromPart` now calls it (`statusForToolState(part.state as AdapterToolState)`); the `AdapterToolState` parameter type keeps the `switch` exhaustive over exactly the four states the adapter emits.