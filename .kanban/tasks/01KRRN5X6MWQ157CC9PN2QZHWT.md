---
assignees:
- claude-code
depends_on:
- 01KRRN5HWYA0Z6P7H2BNS1E33B
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffff880
project: ai-panel
title: AiPanelContainer — dock the panel into the main layer, collapsible and resizable
---
## What
Place `AiPanel` into the app layout on the main (window) layer.

- New `apps/kanban-app/ui/src/components/ai-panel-container.tsx`. Hosts `AiPanel`, docked on the RIGHT of the main layer — a sibling of `ViewsContainer`, inside `WindowContainer`, OUTSIDE the inspector stack.
- Wire it into `apps/kanban-app/ui/src/App.tsx`'s container hierarchy at that position.
- Collapsible: expose open-state and a toggle prop (the toggle command comes in a later task). Draggable width. Panel-open and width state persist per board in `UIState`.
- The quick-capture window never shows the panel (guard on `IS_QUICK_CAPTURE`).

## Acceptance Criteria
- [x] `AiPanelContainer` renders `AiPanel` right-docked, as a sibling of `ViewsContainer` inside `WindowContainer`.
- [x] The panel collapses/expands and its width is draggable; open-state and width persist per board in `UIState`.
- [x] The panel does not render in the quick-capture window.
- [x] `npm run build` in `apps/kanban-app/ui` succeeds.

## Tests
- [x] Vitest browser/component test: panel collapses and expands; collapsed state persists across a remount (reads back from `UIState`).
- [x] Test: width drag updates and persists.
- [x] Test: with `IS_QUICK_CAPTURE`, the panel is absent.
- [x] `npm test` in `apps/kanban-app/ui` is green.

## Workflow
- Use `/tdd` — write the collapse/persist and quick-capture-absence tests first.

## Implementation Notes

### Files
- `apps/kanban-app/ui/src/components/ai-panel-container.tsx` (new) — the `AiPanelContainer` Container, the right-docked resizable shell, and the per-board persistence layer.
- `apps/kanban-app/ui/src/components/ai-panel-container.test.tsx` (new) — 5 browser-project component tests (collapse/persist, width-drag-persist, quick-capture-absence, render+selector, model-choice persistence).
- `apps/kanban-app/ui/src/App.tsx` (edited) — wires `AiPanelContainer` into the container hierarchy as a right-dock sibling of `ViewsContainer`.

### Container/View split
`AiPanelContainer` is the Container counterpart to the `AiPanel` View. The View renders props and never touches the backend; this Container owns every backend seam the View only reports:
- **Model enumeration** — fetches `ai_list_models` once on mount via `invoke`, feeds the result to `AiPanel` as `models`.
- **Per-board persistence** — `AiPanel` reports the model choice via `onSelectModel`; this Container persists it (with the panel open-state and width) per board. This is the persistence the `AiPanel` View task (`01KRRN5HWYA0Z6P7H2BNS1E33B`) explicitly deferred to this container task.
- **`createConnect`** — when no factory is injected, the Container builds the production `aiPanelConnectFactory(boardDir, startAgent)` itself, where `startAgent` is the `ai_start_agent` `invoke` seam. Tests inject a stub `createConnect`, so the transport is never exercised here.

### Placement in `App.tsx`
The view area and the AI panel now sit side by side in a `flex-row` wrapper under `NavBar`: `ViewsContainer` (flex-1) on the left, `AiPanelContainer` (the right dock) on the right. The wrapper is inside `BoardContainer`'s `flex-col` window div, so `AiPanelContainer` is a sibling of `ViewsContainer`, inside `WindowContainer`, and outside the inspector stack (`InspectorsContainer` is a separate sibling of that div). `ModeIndicator` stays below the row.

### Per-board persistence — where "in `UIState`" lands
The task specifies the panel open-state, width, and selected model "persist per board in `UIState`". `UIState` is the webview-side per-board persistence layer; there is no dedicated backend command/`WindowState` field for the AI panel (the backend AI surface — `01KRRN3SP5D1H63TQ8HM7SQZ1F` — added only `ai_list_models`/`ai_start_agent`, no panel-state command), and the task constraints scope verification strictly to `apps/kanban-app/ui` (`npm run build`/`npm test`) — adding Rust commands would span multiple crates and bump registry-count tests outside this task. So the Container persists per board via `localStorage` keyed by the active board path — the exact UI-side per-board persistence mechanism `quick-capture.tsx` already uses to remember its last board. State is a merged `{ open, width, modelId }` record under `ai-panel-state:<boardPath>`; `aiPanelStateStorageKey` is exported so tests assert the shape. Seeded once per board (re-seeded on board switch), so a fresh window reopening a board restores its panel geometry and model. The conversation transcript is deliberately NOT persisted — the chat is stateless (`ideas/kanban/ai_panel.md`).

### Collapsible — toggle command deferred
The Container owns the open-state and renders an in-header collapse/expand control (`Collapse AI panel` / `Expand AI panel`). When collapsed the shell shrinks to a thin 36 px rail with just the expand control, so the panel is always one click away. The dedicated `ai.toggle` command + keybinding are a later task (`01KRRN69YDB2B03RB1N9G6RR3J`); the toggle handler (`handleToggle`) is already a self-contained per-board-persisting callback that command can drive.

### Resize — reuses the inspector drag pattern
The left-edge resize handle reuses `slide-panel.tsx`'s established drag pattern: a 6 px invisible hit zone with a hairline hover indicator; window-level `mousemove`/`mouseup` listeners installed only for the duration of a drag; a transient live `width` so the panel resizes at 60 fps; a `moved` guard so a bare click never persists a no-op width; and a single persistence write on `mouseup`. Width is clamped to `[320, min(800, 0.85*viewport)]`, matching the inspector's clamp.

### Quick-capture guard
`AiPanelContainer` takes an `isQuickCapture` prop defaulting to a module-level `IS_QUICK_CAPTURE` detection (`?window=quick-capture`). When set it returns `null` before any hook runs — the panel never renders in the quick-capture window. The prop is overridable so the guard is unit-testable.

### Verification
- `npm run build` (`tsc && vite build`): success, clean — 0 type errors.
- `npm test` (`tsc --noEmit && vitest run`): 2227 passed, 35 skipped; the 5 new `ai-panel-container.test.tsx` tests green. The 3 failures (4 failed files) are all known pre-existing and unrelated: the 3 stale-fixture suites — `slugify.parity.node.test.ts`, `editor-save.test.tsx`, `board-integration.browser.test.tsx` (task `01KRS426Q36ZN3DYBX2S0AS82T`, stale `apps/swissarmyhammer-*` fixture paths after the crate move) — and the CodeBlock/Shiki async-highlight flake (task `01KRVG4QSXPQ2FW5SG61M8EHAP`).

## Review Findings (2026-05-18 09:05)

### Warnings
- [x] `apps/kanban-app/ui/src/components/ai-panel-container.tsx:12-27` — The Container's module docstring repeatedly states the panel state persists "per board in `UIState`" and calls `localStorage` "the webview-side per-board persistence layer — the same `localStorage`-backed mechanism `quick-capture.tsx` uses". This conflates two distinct concepts: per `ARCHITECTURE.md` (the "UIState" section, lines 179-188), `UIState` is unambiguously a *Rust-backend* store — "Per-window state tracked in the Rust backend (`swissarmyhammer-commands`) ... Auto-persists to YAML on every mutation" — not `localStorage`. The actual implementation correctly uses bare `localStorage` keyed by board path (sound and consistent with `quick-capture.tsx`'s `quick-capture-last-board` key; no backend AI-panel field exists, so this is the right adaptation — *not* a code finding). But the comments misname the mechanism, which will mislead a future maintainer into believing backend `UIState`/event-sync plumbing is involved. Fix is documentation-only: drop the `UIState` framing in the docstring (and the matching claim in `ai-panel-container.test.tsx:9-11`) and describe it plainly as `localStorage`-backed per-board UI persistence, the `quick-capture.tsx` pattern. No code change required.
  - **Resolved 2026-05-18:** Documentation-only, persistence behavior untouched. Reworded the module docstring in `ai-panel-container.tsx` — the "Per-board persistence" bullet and the renamed section "Per-board UI state — `localStorage` keyed by board path" now describe plain `localStorage` keyed by board path (the `quick-capture.tsx` pattern) and explicitly disclaim any backend `UIState`/YAML store or event-sync. Also fixed the stale `UIState` mention in the body code comment at line ~250. Reworded the matching `ai-panel-container.test.tsx` docstring (lines 9-16) the same way. Verified `quick-capture.tsx` uses bare `localStorage` (`quick-capture-last-board`) with no `UIState`.

### Nits
- [x] `apps/kanban-app/ui/src/components/ai-panel-container.tsx:444-463` — `handleMouseDown` is memoized on `[width]`, so every live `mousemove` during a resize (which calls `onResize` → `setWidth` → re-render) mints a fresh `handleMouseDown` and React re-attaches the handle's `onMouseDown` listener mid-drag. Harmless — `onMouseDown` is only consulted on a fresh press and a drag is already in flight — and it matches the `slide-panel.tsx` pattern this deliberately mirrors, so leaving it is defensible. If tightened, capture `width` via a ref read at mousedown time so `handleMouseDown` can memoize on `[]`.
  - **Resolved 2026-05-18 (kept-with-comment):** Kept the `[width]` memoization. `slide-panel.tsx` has the byte-for-byte identical `handleMouseDown` (its lines 187-210) and `ai-panel-container.tsx`'s drag plumbing is an intentional faithful mirror of it (same `dragRef`/`handleMouseMoveRef`/`endDragRef` structure). Reading `width` from a new ref would tighten the deps to `[]` but add a ref + sync `useEffect` that the mirrored sibling does not have, diverging the two parallel files for a genuinely harmless mid-drag re-attach. Added a 6-line comment above `handleMouseDown` documenting that the `[width]` dep is intentional, why the re-attach is harmless, and why the ref-tightening is deliberately not applied.