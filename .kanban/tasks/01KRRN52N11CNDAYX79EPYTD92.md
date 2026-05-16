---
assignees:
- claude-code
depends_on:
- 01KRRN4P0CN17AWCV2TQAF0NC1
position_column: todo
position_ordinal: '8780'
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
- [ ] Each `SessionUpdate` variant updates the conversation store to the documented `UIMessage` part shape.
- [ ] Streaming text/thought chunks coalesce into a single growing part.
- [ ] Turn status reflects streaming vs idle vs error; "new conversation" clears the store.
- [ ] The hook exposes `messages` as `UIMessage[]` directly consumable by AI Elements — no chat-framework dependency.
- [ ] `npm run build` in `apps/kanban-app/ui` succeeds.

## Tests
- [ ] Vitest unit tests: feed each `SessionUpdate` variant, assert the resulting message state (one test per variant).
- [ ] Test chunk coalescing and turn-status transitions.
- [ ] `npm test` in `apps/kanban-app/ui` is green.

## Workflow
- Use `/tdd` — write the per-variant translation tests first.