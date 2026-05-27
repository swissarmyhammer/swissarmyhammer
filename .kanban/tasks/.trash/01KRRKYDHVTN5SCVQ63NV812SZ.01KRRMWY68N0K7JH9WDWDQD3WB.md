---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: '8980'
project: ai-panel
title: Vendor AI Elements components + TauriChatTransport
---
## What
Bring AI Elements into the webview and build the transport that bridges the AI SDK `useChat` hook to the Tauri backend.

- Add the `ai` SDK package to `apps/kanban-app/ui/package.json`. Install AI Elements components (the AI Elements CLI / shadcn-style copy-in) into `apps/kanban-app/ui/src/components/ai-elements/` — at minimum `Conversation`, `Message`, `Response`, `Reasoning`, `Tool`, `Task`, `PromptInput`, `Loader`, `Actions`.
- Create `apps/kanban-app/ui/src/ai/tauri-chat-transport.ts` — `TauriChatTransport implements ChatTransport<UIMessage>` (the AI SDK transport interface — NOT ACP; the webview never speaks ACP):
  - `sendMessages` -> `invoke("ai_send_prompt", { windowLabel, text })`, returns a `ReadableStream<UIMessageChunk>` fed by Tauri events `ai://chunk/{windowLabel}` (content parts) and `ai://status/{windowLabel}` (finish/error).
  - `abortSignal` -> `invoke("ai_cancel_prompt", ...)`.
  - `reconnectToStream` -> unsupported (stateless chat).
- The backend already shapes `ai://chunk` events as `UIMessageChunk` parts; the transport is a thin event-to-stream adapter.

Spec: `ideas/kanban/ai_panel.md` — Phase 4 "AI Elements", "Transport — useChat over Tauri".

## Acceptance Criteria
- [ ] AI Elements components are vendored under `ui/src/components/ai-elements/` and type-check.
- [ ] `TauriChatTransport` implements the AI SDK `ChatTransport` interface and is usable as the `transport` for `useChat`.
- [ ] `sendMessages` produces a stream sourced from `ai://chunk` events and terminates on `ai://status`.
- [ ] `npm run build` (tsc + vite) in `apps/kanban-app/ui` succeeds.

## Tests
- [ ] Vitest unit test for `TauriChatTransport`: mock Tauri `invoke`/`listen`; emit a sequence of `ai://chunk` events then `ai://status`; assert `sendMessages` yields the corresponding `UIMessageChunk` stream and closes.
- [ ] Test that `abortSignal` triggers `ai_cancel_prompt`.
- [ ] `npm test` (`tsc --noEmit && vitest run`) in `apps/kanban-app/ui` is green.

## Workflow
- Use `/tdd` — write the transport unit test against mocked Tauri APIs first.