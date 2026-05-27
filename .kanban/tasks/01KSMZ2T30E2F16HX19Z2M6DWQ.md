---
assignees:
- claude-code
depends_on:
- 01KSMZ1Y2CWKJE32PG16T328K7
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffb080
project: ai-panel
title: AI panel reads `board.model`, dispatches `update.board` on selection
---
## What

Stop persisting the AI panel's selected model in `localStorage`. Source it from the board entity (which now carries `model` after the previous task), and write it back via the `update.board` command. `open` and `width` (UI geometry) stay in `localStorage` — only `model` moves.

**No migration.** Per the user, drop localStorage `modelId` and start fresh — the existing auto-default-selection logic in `AiPanelContainerBody` already covers "no model picked yet."

### Files

- `apps/kanban-app/ui/src/components/ai-panel-container.tsx`
  - Removed `modelId?: string` from the `AiPanelState` interface.
  - Removed the `modelId` from `loadAiPanelState`'s return shape.
  - In `AiPanelContainerBody`:
    - Dropped the `useState<string | null>` seeded from `persisted.modelId`.
    - Source `modelId` from the active board entity via `useBoardData()` and `getStr(board.board, "model")`.
    - Dropped the `setModelId(next.modelId ?? null)` line from the board-switch `useEffect`.
  - In `handleSelectModel`:
    - Stopped calling `saveAiPanelState(boardPath, { modelId: id })`.
    - Dispatches `update.board` via `useDispatchCommand("update.board")` with `{ args: { model: id } }`.
    - Keeps an optimistic local state for instant UI feedback (cleared on board switch and when the board entity catches up).
  - Auto-default-selection still works — it goes through `handleSelectModel`, which now writes through `update.board`.

## Acceptance Criteria

- [x] `AiPanelState` interface no longer has a `modelId` field.
- [x] No code path under `apps/kanban-app/ui/src/components/ai-panel-container.tsx` writes `modelId` to `localStorage`.
- [x] Picking a model in `ComposerModelSelect` dispatches `update.board` with `{ model: <id> }`.
- [x] On board switch, the picker reflects the new board's `model` (or falls through auto-default-selection if unset). No state from the previous board persists.
- [x] `open` and `width` are still persisted per-board in `localStorage`.

## Tests

Updated `apps/kanban-app/ui/src/components/ai-panel-container.test.tsx`:

- [x] Replaced the `modelId in localStorage` assertions with: selecting a model invokes the `update.board` dispatcher with `{ model: <id> }`.
- [x] Test: `selected model rehydrates from board.model on mount` — board entity has `model: "qwen-coder"`, the picker's `selectedModel` ends up as `qwen-coder`.
- [x] Test: `switching boards swaps in the new board's model` — render with board A (`model: qwen-coder`), switch to board B (`model: claude-code`), assert the picker reflects `claude-code`.
- [x] Test: `board with no model triggers auto-default-selection` — auto-pick now goes through `update.board` instead of localStorage.

Test results: all 17 ai-panel-container tests pass; the full `apps/kanban-app/ui` suite passes (257 files, 2450 tests).

## Workflow

- Used `/tdd` — wrote failing test assertions first, then refactored.