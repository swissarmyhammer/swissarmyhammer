---
assignees:
- claude-code
depends_on:
- 01KSMZ1Y2CWKJE32PG16T328K7
position_column: todo
position_ordinal: '8480'
project: ai-panel
title: AI panel reads `board.model`, dispatches `update.board` on selection
---
## What

Stop persisting the AI panel's selected model in `localStorage`. Source it from the board entity (which now carries `model` after the previous task), and write it back via the `update.board` command. `open` and `width` (UI geometry) stay in `localStorage` — only `model` moves.

**No migration.** Per the user, drop localStorage `modelId` and start fresh — the existing auto-default-selection logic in `AiPanelContainerBody` already covers "no model picked yet."

### Files

- `apps/kanban-app/ui/src/components/ai-panel-container.tsx`
  - Remove `modelId?: string` from the `AiPanelState` interface (around line 117–123).
  - Remove the `modelId` line in `loadAiPanelState`'s return shape — `AiPanelState` no longer has it.
  - In `AiPanelContainerBody` (around line 254–275):
    - Drop the `useState<string | null>` seeded from `persisted.modelId`.
    - Source `modelId` from the active board entity instead. The board store already loads `GetBoard`'s response (see `apps/kanban-app/ui/src/lib/refresh.test.ts` and `board-data-sync.test.tsx` for the existing board-entity flow). Read `board.fields.model` (or whatever the existing accessor pattern is — match how `name`/`description` are read elsewhere; check `nav-bar.tsx` for a precedent).
    - Drop the `setModelId(next.modelId ?? null)` line from the board-switch `useEffect`. Switching boards naturally swaps in the new board's `model` because the source changed.
  - In `handleSelectModel` (around line 295–301):
    - Stop calling `saveAiPanelState(boardPath, { modelId: id })`.
    - Dispatch `update.board` with `{ model: id }`. Use `useDispatchCommand("update.board")` — see `feedback_useDispatchCommand_signature` memory: the hook takes a **command name string**, not a command object. The hook returns a dispatcher; call it with the parameter payload.
    - Keep the local optimistic state update if the existing pattern relies on it for instant UI feedback; otherwise let the board store push the change back.
  - Auto-default-selection (the `useEffect` that picks the first available model when none is set) keeps working unchanged — it already dispatches through `handleSelectModel`, which now writes through `update.board`.

### Things to check before editing

- Confirm `update.board` (the kanban command verb/noun pair from `crates/swissarmyhammer-kanban/src/board/update.rs`) is exposed in the frontend command registry. If it isn't already wired to a YAML command def + the dispatcher path, refer to memory: `reference_adding_commands.md`. Most likely it is already registered since `UpdateBoard` is an `#[operation]` and the kanban-app picks up all kanban operations.
- Confirm the board-entity accessor for reading custom fields like `model` — match the pattern that reads `description` (which `GetBoard` already returns). If `description` is consumed somewhere in the UI, mirror that path.

## Acceptance Criteria

- [ ] `AiPanelState` interface no longer has a `modelId` field.
- [ ] No code path under `apps/kanban-app/ui/src/components/ai-panel-container.tsx` writes `modelId` to `localStorage`.
- [ ] Picking a model in `ComposerModelSelect` dispatches `update.board` with `{ model: <id> }`.
- [ ] On board switch, the picker reflects the new board's `model` (or falls through auto-default-selection if unset). No state from the previous board persists.
- [ ] `open` and `width` are still persisted per-board in `localStorage`.

## Tests

Update `apps/kanban-app/ui/src/components/ai-panel-container.test.tsx`:

- [ ] Replace the `modelId in localStorage` assertions with: selecting a model invokes the `update.board` dispatcher with `{ model: <id> }`.
- [ ] Test: `selected model rehydrates from board.model on mount` — board entity has `model: "qwen"`, the picker's `selectedModel` ends up as `qwen`.
- [ ] Test: `switching boards swaps in the new board's model` — render with board A (`model: qwen`), switch to board B (`model: claude-code`), assert the picker reflects `claude-code`.
- [ ] Test: `board with no model triggers auto-default-selection` — existing test should still pass; verify it does and that the auto-pick now goes through `update.board` instead of localStorage.

Run: `cd apps/kanban-app/ui && pnpm test ai-panel-container`.

Also run the broader kanban-app frontend tests to catch any board-store consumers that incidentally depended on `modelId` in localStorage:

- [ ] `cd apps/kanban-app/ui && pnpm test`

## Workflow

- Use `/tdd` — write the failing test assertions first, then refactor.