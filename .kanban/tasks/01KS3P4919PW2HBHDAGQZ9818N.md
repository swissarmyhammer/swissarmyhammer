---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8c80
project: ai-panel
title: Auto-select a default AI model so the panel is never stuck in "no model" state
---
## What

The AI panel can land in a dead-end state where the conversation surface is visible but unusable: the body renders `NoModelState` ("Choose a model") with a disabled composer, and the user has to spelunk into the composer footer's model picker before they can chat. That happens whenever a board has no persisted `modelId` — i.e. every fresh board, and any board whose `localStorage` AI-panel snapshot is cleared. There is no good reason to show this state when `ai_list_models` already returned a usable model.

Fix: when `ai_list_models` resolves and the current per-board `modelId` is still `null`, auto-select a sensible default and persist it for the board — the same path the user would take through `handleSelectModel` if they clicked the picker. The default is the **first `available: true` model** in the list returned by `ai_list_models` (Claude Code is synthesized first, so it wins when its CLI is detected; otherwise the first available local llama model — `apps/kanban-app/src/ai/models.rs::ai_list_models`). If no model is available at all, leave `modelId` as `null` and continue to render `NoModelState` with its existing "No AI models are configured." copy — that's a genuine empty-config case, not a dead-end.

The selection runs only when `modelId === null` so it never overrides a user's prior pick, and it persists through `saveAiPanelState` so the next reopen of the board is already configured. The effect must depend on `[models, modelId, handleSelectModel]` so a late-arriving model list still triggers the default, but an explicit user pick (which sets `modelId` to a non-null value) blocks the auto-selection from re-firing.

### Files to modify

- `apps/kanban-app/ui/src/components/ai-panel-container.tsx` — in `AiPanelContainerBody`, after the `ai_list_models` effect, add a `useEffect` that runs when `models` is loaded and `modelId === null`: picks `models.find(m => m.available)?.id`, and if present calls `handleSelectModel(id)`. Document the rule in a short JSDoc block on the new effect.

### Files to add tests in

- `apps/kanban-app/ui/src/components/ai-panel-container.test.tsx` — new tests against `AiPanelContainerBody` exercising the auto-select branch via the existing `ai_list_models` mock pattern. Reuse the existing harness — do not introduce a parallel test setup.

### Out of scope

- The `NoModelState` view itself (copy, layout, focus) — that stays as the fallback for the genuine "no available models" case.
- Cross-board defaults / "remember the last model across boards" — this task is per-board only, picking the first available model on first ever load for that board.
- Backend changes to `ai_list_models` — ordering is already correct (Claude Code first, then local llamas).

## Acceptance Criteria

- [x] On a fresh mount where `useActiveBoardPath` returns a path with no persisted `ai-panel-state:<path>` entry, after `ai_list_models` resolves with at least one `available: true` model, the per-board `modelId` becomes that model's id within one render and is persisted via `saveAiPanelState` so a remount reads it back from `localStorage`.
- [x] When `ai_list_models` resolves but every entry has `available: false`, `modelId` stays `null` and `AiPanel` continues to render `NoModelState` — the auto-select must not pick an unavailable model.
- [x] When a board already has a persisted `modelId`, the auto-select effect is a no-op: the persisted id is honored and never overwritten by the default-selection logic, even if the persisted model is `available: false` (the user's explicit prior choice wins).
- [x] A subsequent user pick through `handleSelectModel` continues to work and persist exactly as before; no new code paths fire on user-initiated selections.

## Tests

- [x] `apps/kanban-app/ui/src/components/ai-panel-container.test.tsx` — new test: mounts `AiPanelContainer` for a board with no persisted state, stubs `ai_list_models` to return `[{ id: "claude-code", available: true, ... }, { id: "qwen-coder", available: true, ... }]`, awaits a microtask flush, then asserts the rendered `AiPanel` receives `modelId === "claude-code"` and `localStorage.getItem(aiPanelStateStorageKey(boardPath))` parses to `{ modelId: "claude-code", ... }`.
- [x] `apps/kanban-app/ui/src/components/ai-panel-container.test.tsx` — new test: same setup but `ai_list_models` returns every entry with `available: false`; assert `modelId` stays `null`, the `NoModelState` empty state renders (stable "Choose a model" heading; specific copy below it is owned by NoModelState — out of scope), and nothing is written to `localStorage` for that board's modelId field.
- [x] `apps/kanban-app/ui/src/components/ai-panel-container.test.tsx` — new test: pre-seed `localStorage` with `{ modelId: "qwen-coder" }`, mount with `ai_list_models` returning a list whose first available entry is `claude-code`; assert the rendered `AiPanel` receives `modelId === "qwen-coder"` (auto-select did not override the persisted pick).
- [x] `cd apps/kanban-app/ui && npm test -- ai-panel-container` passes (13/13).

## Workflow
- Use `/tdd` — write the three failing tests first, then add the auto-select effect to `AiPanelContainerBody` and make them pass.

## Implementation notes

- The new `useEffect` was placed directly after `handleSelectModel` so it can route through the same persistence path the user pick uses. `handleSelectModel` itself was hoisted slightly above `handleToggle` (its prior position) so the auto-select effect can reference it — no other behavior change.
- Two existing tests had to be updated because the auto-select default is now part of the contract:
  - "renders AiPanel right-docked with the model selector" — selector accessible name is now the picked label ("Claude Code") rather than the "Select a model" placeholder.
  - "persists and reapplies the per-board model choice" — rewritten to use a two-model fixture so the user picks the *second* model, overriding the auto-selected default; verifies user picks still override and persist.
- Test 2 ("all unavailable") asserts the stable NoModelState marker ("Choose a model" heading + no persisted modelId), not the specific empty-state copy line. The copy branches inside NoModelState on `models.length > 0` not `models.some(available)`, and reworking that branching is explicitly out of scope.