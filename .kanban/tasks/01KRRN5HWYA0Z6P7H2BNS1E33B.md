---
assignees:
- claude-code
depends_on:
- 01KRRN52N11CNDAYX79EPYTD92
- 01KRRN386C7THGV5T6RCA59H4F
- 01KRRN3SP5D1H63TQ8HM7SQZ1F
position_column: todo
position_ordinal: '8880'
project: ai-panel
title: AiPanel component — conversation UI, model selector, permission prompt, composer
---
## What
The `AiPanel` React component — the conversation surface.

- New `apps/kanban-app/ui/src/components/ai-panel.tsx`. Render the conversation store (the `sessionUpdate` task) with the vendored AI Elements components: `Conversation` (autoscroll), `Message`, `Response` (streamed assistant markdown), `Reasoning` (collapsible thinking), `Tool` (kanban tool-call cards: name, args, status, result), `Task` (the agent's plan), `Loader`.
- Model selector in the panel header from `ai_list_models`; unavailable entries disabled with a hint. Selecting a model: call `ai_start_agent(model_id)`, open the WebSocket ACP stream, construct the TypeScript ACP client, start a fresh stateless session. The choice persists per board in `UIState`.
- Permission prompt: when the ACP client's `requestPermission` callback fires, render an inline approval UI in the conversation (allow once / allow for session / deny); the user's choice resolves the `requestPermission` promise.
- Composer (`PromptInput`): submit calls `prompt` on the ACP client; the stop button calls `cancel`.
- "New conversation" tears down the session and starts a fresh one.

## Acceptance Criteria
- [ ] `AiPanel` renders the conversation from ACP session updates: streamed text, reasoning, tool-call cards, plan.
- [ ] The model selector lists `ai_list_models` entries, disables unavailable ones, persists the choice per board, and switching starts a fresh ACP session.
- [ ] A `requestPermission` callback renders an inline prompt whose result resolves the ACP request.
- [ ] The composer drives `prompt`/`cancel`.
- [ ] `npm run build` in `apps/kanban-app/ui` succeeds.

## Tests
- [ ] Vitest browser/component test: render `AiPanel` with a mock ACP client; drive `sessionUpdate` callbacks; assert assistant text, a `Reasoning` block, and a `Tool` card render.
- [ ] Component test: selector lists models, disables unavailable, persists choice.
- [ ] Component test: a `requestPermission` callback renders the prompt and a click resolves it with the chosen option.
- [ ] `npm test` in `apps/kanban-app/ui` is green.

## Workflow
- Use `/tdd` — write the component tests against a mock ACP client first.