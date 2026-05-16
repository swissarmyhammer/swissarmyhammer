---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: 8a80
project: ai-panel
title: AiPanel component — conversation UI, model selector, permission prompt
---
## What
Build the `AiPanel` React component — the conversation surface itself.

- Create `apps/kanban-app/ui/src/components/ai-panel.tsx`. Use the AI SDK `useChat` hook with the `TauriChatTransport`.
- Render with AI Elements: `Conversation` (autoscroll), `Message` rows, `Response` (streamed assistant markdown), `Reasoning` (collapsible thinking), `Tool` (kanban tool-call cards: name, args, status, result), `Task` (the agent's plan), `Loader` (streaming indicator).
- Model selector in the panel header: a dropdown sourced from `ai_list_models`; disabled entry + hint when a model is unavailable. Selecting a model starts a fresh session (stateless) — calls `ai_start_session`. The chosen model is persisted per board (`UIState`).
- Permission prompt: render an inline approval UI in the conversation when an `ai://permission` event arrives (allow once / allow for session / deny), replying via `ai_respond_permission`.
- Stateless: each panel open / "New conversation" starts a fresh session; nothing is persisted except the model choice and panel layout.

Spec: `ideas/kanban/ai_panel.md` — Phase 3 "The selector", Phase 4 "AI Elements", "Layout & placement".

## Acceptance Criteria
- [ ] `AiPanel` renders a `useChat` conversation through `TauriChatTransport`: streamed text, reasoning, tool-call cards, and plan all display.
- [ ] The model selector lists `ai_list_models` entries; unavailable entries are disabled with a hint; selection persists per board.
- [ ] An `ai://permission` event renders an inline approval prompt that replies via `ai_respond_permission`.
- [ ] `npm run build` in `apps/kanban-app/ui` succeeds.

## Tests
- [ ] Vitest browser/component test: render `AiPanel` with a mock `TauriChatTransport`; drive a chunk stream; assert assistant text, a `Reasoning` block, and a `Tool` card render.
- [ ] Component test: selector lists models, disables unavailable ones, and persists the choice.
- [ ] Component test: a permission event renders the approval prompt and a click dispatches `ai_respond_permission`.
- [ ] `npm test` in `apps/kanban-app/ui` is green.

## Workflow
- Use `/tdd` — write the component tests against a mock transport first.