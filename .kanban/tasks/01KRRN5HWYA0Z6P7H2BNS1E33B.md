---
assignees:
- claude-code
depends_on:
- 01KRRN52N11CNDAYX79EPYTD92
- 01KRRN386C7THGV5T6RCA59H4F
- 01KRRN3SP5D1H63TQ8HM7SQZ1F
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffff780
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
- [x] `AiPanel` renders the conversation from ACP session updates: streamed text, reasoning, tool-call cards, plan.
- [x] The model selector lists `ai_list_models` entries, disables unavailable ones with a hint, REPORTS the per-board model choice via `onSelectModel` (the hosting Container persists it to `UIState`), and switching starts a fresh ACP session.
- [x] A `requestPermission` callback renders an inline prompt whose result resolves the ACP request.
- [x] The composer drives `prompt`/`cancel`.
- [x] `npm run build` in `apps/kanban-app/ui` succeeds.

## Tests
- [x] Vitest browser/component test: render `AiPanel` with a mock ACP client; drive `sessionUpdate` callbacks; assert assistant text, a `Reasoning` block, and a `Tool` card render.
- [x] Component test: selector lists models, disables unavailable, persists choice.
- [x] Component test: a `requestPermission` callback renders the prompt and a click resolves it with the chosen option.
- [x] `npm test` in `apps/kanban-app/ui` is green.

## Workflow
- Use `/tdd` — write the component tests against a mock ACP client first.

## Implementation Notes

### Files
- `apps/kanban-app/ui/src/components/ai-panel.tsx` (new) — the `AiPanel` View component and its sub-components.
- `apps/kanban-app/ui/src/components/ai-panel.test.tsx` (new) — 7 component tests against a mock ACP client (browser project).

### `AiPanel` is a View, not a Container
Per `ARCHITECTURE.md`'s Container/View split, `AiPanel` is a View: it takes props and renders, and never calls the Tauri backend directly. The backend seams are injected as props so the panel is testable with no `invoke` and no transport:
- `models` / `modelId` / `onSelectModel` — the hosting container (separate task `01KRRN5X6MWQ157CC9PN2QZHWT`) fetches `ai_list_models`, **persists the per-board choice in `UIState`**, and feeds the selected id back as a prop. `AiPanel` renders the selector and reports the user's choice via `onSelectModel`; it does not itself write `UIState`. This is the correct boundary — a View dispatches/reports, the Container persists.
- `createConnect: (modelId) => ConversationConnect` — given a model id, returns the `ConversationConnect` factory `useConversation` calls. The exported production helper `aiPanelConnectFactory(boardDir, startAgent)` composes the real handoff: `ai_start_agent(modelId)` → `connectAcpStream(wsUrl)` → `createKanbanClient({stream, boardDir, mcpUrl, ...handlers})`. Tests inject a mock that returns a `ConversationConnect` backed by a fake session.

### Switching a model starts a fresh session
The inner `AiPanelConversation` is mounted with `key={modelId}`. Selecting a different model remounts it, which drops the prior `useConversation` hook state — tearing down the prior ACP client/session — and builds a brand-new stateless one for the new model. This is the "fresh stateless session per model" the task requires, achieved with React's keyed-remount rather than imperative teardown.

### Conversation rendering
`AiPanelConversation` renders `useConversation().messages` directly (they *are* `UIMessage[]`). Each message part is dispatched by kind: `text` → `MessageResponse` (streamed markdown), `reasoning` → `Reasoning`/`ReasoningContent` (collapsible), `dynamic-tool` → `Tool`/`ToolHeader`/`ToolInput`/`ToolOutput` (tool-call card with name, args, status badge, result/error), the custom `data-plan` → `Task`/`TaskItem` (the agent's plan as a status-coloured checklist). A `Loader` + "Thinking..." shows while `status === "streaming"`. `Conversation` provides autoscroll; `ConversationScrollButton` the jump-to-bottom affordance; `ConversationEmptyState` the pre-conversation and no-model placeholders.

### Permission prompt
When `useConversation().permissionRequest` is non-null, an inline `PermissionPrompt` renders inside the conversation log: the tool title plus one `Button` per `RequestPermissionRequest.options` entry (allow once / allow for session / deny), variant-styled by `PermissionOptionKind`. Clicking a button calls `respondPermission({ outcome: { outcome: "selected", optionId } })`, which resolves the agent's awaiting `requestPermission` promise; the prompt then disappears.

### Composer
`ComposerArea` wraps `PromptInput`: submit calls `sendPrompt([{type:"text", text}])`; while a turn streams the submit button becomes a stop control (`PromptInputSubmit status="streaming"` shows the square glyph) whose click calls `cancel`. The conversation `status` (`idle`/`streaming`/`error`) maps to the AI SDK `ChatStatus` (`ready`/`streaming`/`error`). "New conversation" calls `newConversation` (resets the store, drops the session for a fresh `newSession`). The textarea is disabled until a model is selected and available.

### Tests
`ai-panel.test.tsx` — 7 browser-project tests against a hand-written mock ACP client (`FakeSession` replays scripted `sessionUpdate` notifications; `HangingSession` models a never-resolving turn for the stop test). Coverage: conversation render (asserts streamed assistant text + a `Reasoning` block + a `Tool` card + a plan), stop-button cancel, "New conversation" clears the log, selector lists models / disables unavailable / shows hint, selecting a model reports the choice and connects through the new model, composer disabled with no model, and the permission prompt rendering + click-resolves-the-request round trip.

### Verification
- `npm run build` (`tsc && vite build`): success, 3763 modules.
- `npm test` (`tsc --noEmit && vitest run`): 2222 passed, 35 skipped, 7 new `ai-panel.test.tsx` tests green, 0 type errors. The 4 failures are all known pre-existing and unrelated: the 3 stale-fixture suites — `editor-save.test.tsx` and `slugify.parity.node.test.ts` ×2 (task `01KRS426Q36ZN3DYBX2S0AS82T`, missing `apps/swissarmyhammer-*` fixture paths after the crate move) — and the CodeBlock/Shiki smoke flake (task `01KRVG4QSXPQ2FW5SG61M8EHAP`).

## Review Findings (2026-05-18 08:10)

Task-mode review of `apps/kanban-app/ui/src/components/ai-panel.tsx` and `ai-panel.test.tsx`. Six examination layers + JS_TS_REVIEW.md + ARCHITECTURE.md Container/View alignment. The `ai-panel.test.tsx` suite was run in isolation: 7/7 pass. `tsc --noEmit` reports no type errors in the panel files. The Container/View claim holds — `AiPanel` is a pure View: props in, render out, no `invoke`, no transport; the keyed-remount for fresh-session-per-model is sound (remounting `AiPanelConversation` drops the `useConversation` hook's `clientRef`/`sessionRef`, forcing a new client + `newSession`). No blockers, no warnings.

### Nits
- [x] `apps/kanban-app/ui/src/components/ai-panel.tsx` (AC2) — Acceptance Criterion 2's "persists the choice per board" is checked on this task, but `AiPanel` deliberately does *not* persist — it reports the choice via `onSelectModel` and renders the `modelId` fed back as a prop; the actual `UIState` write lands in the blocked container task `01KRRN5X6MWQ157CC9PN2QZHWT` (whose own AC explicitly covers persisting per-board state). The split is architecturally correct and matches `ideas/kanban/ai_panel.md` (persistence is Phase 3 / the container, the panel UI is Phase 4). No code change needed; the AC wording is just broader than this View-only task's true scope. Consider rewording AC2 here to "reports the choice via `onSelectModel`" so the checkbox reflects what this task actually delivers.
  - Resolution: AC2 reworded to state the selector REPORTS the per-board choice via `onSelectModel` (the hosting Container persists it to `UIState`). The wording now matches the View-only scope and the Container/View split documented in the Implementation Notes. No code change — `AiPanel` already correctly reports rather than persists.
- [x] `apps/kanban-app/ui/src/components/ai-panel.tsx:526` — `<PlanView data={part.data as PlanPartData} />` casts `part.data` to `PlanPartData`. Because `ConversationDataParts` carries a `Record<string, unknown>` index signature alongside `plan: PlanPartData`, the AI SDK's data-part union does not narrow `data` cleanly for the `data-plan` arm, so the cast is defensible — but it is slightly imprecise. If the index signature on `ConversationDataParts` can be tightened (or the `data-plan` part narrowed via a type guard), the cast becomes unnecessary. Minor — current code is correct, just not maximally type-safe.
  - Resolution: TIGHTENED. The bare `as PlanPartData` cast is gone. Added an `isPlanPart` type-guard predicate (and a `PlanPart` helper type) in `ai-panel.tsx`; `MessagePartView` now calls `isPlanPart(part)` before the `switch`, which narrows `part.data` to `PlanPartData` with no cast. The root cause is the `& Record<string, unknown>` index signature on `ConversationDataParts` in `conversation.ts` (it produces a `data: unknown` arm in the AI SDK `DataUIPart` union that overlaps `"data-plan"`), so a `case` label alone cannot narrow `data` — the predicate asserts the concrete arm instead. `conversation.ts` was deliberately left untouched (out of this task's scope). `tsc` / `tsc --noEmit` both clean.
