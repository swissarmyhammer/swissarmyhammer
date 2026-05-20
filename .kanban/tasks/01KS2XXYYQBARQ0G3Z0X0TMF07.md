---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8b80
title: Replace redundant AI header buttons with a single AI star toggle
---
## What

The AI panel header currently shows two redundant controls: a `SparklesIcon` + "AI" label on the left and a `PanelRightCloseIcon` collapse button on the right. The collapsed right rail mirrors the redundancy — a `PanelRightOpenIcon` expand button stacked above a decorative `SparklesIcon`. Consolidate both states to a single AI-star button that toggles the panel.

**Expanded header** — `apps/kanban-app/ui/src/components/ai-panel.tsx` (`AiPanelHeader`, around line 287):
- Remove the left-side `<SparklesIcon /> <span>AI</span>` cluster — no "AI" label.
- Replace the right-side `PanelRightCloseIcon` collapse button's icon with `SparklesIcon` (keep the button, its `onClick={onCollapse}`, ghost styling, and `size="icon"`).
- Update `aria-label` to `"Toggle AI panel"` (it now both opens and closes depending on state — but the expanded-state instance still collapses, so `"Collapse AI panel"` remains correct; use the existing collapse label).
- Drop the unused `PanelRightCloseIcon` import.

**Collapsed rail** — `apps/kanban-app/ui/src/components/ai-panel-container.tsx` (`AiPanelShell` collapsed branch, around line 534):
- Remove the `PanelRightOpenIcon` expand button entirely.
- Replace it with a single `SparklesIcon` button that calls `onToggle`, with `aria-label="Expand AI panel"`, `size="icon"`, `variant="ghost"`.
- Drop the now-decorative second `SparklesIcon` below it.
- Drop the unused `PanelRightOpenIcon` import.

The window-layer `ai.toggle` command (`AppShell` → `triggerAiToggle`) is unchanged — only the visual surface consolidates.

## Acceptance Criteria

- [x] `AiPanelHeader` renders no "AI" text and no `PanelRightCloseIcon`; its right-aligned button uses `SparklesIcon` and keeps `aria-label="Collapse AI panel"`.
- [x] The collapsed rail renders exactly one button — a `SparklesIcon` with `aria-label="Expand AI panel"` — and no `PanelRightOpenIcon`.
- [x] Clicking the star in either state toggles the panel (existing `onToggle`/`onCollapse` wiring preserved).
- [x] No new unused imports; `PanelRightCloseIcon` and `PanelRightOpenIcon` removed from the two files if no other usage remains.

## Tests

- [x] Update `apps/kanban-app/ui/src/components/ai-panel.test.tsx` — the existing header assertions (lines ~461 and ~592 reference the "AI" title); rewrite them to assert the star-only header: no `text("AI")`, one button with `aria-label="Collapse AI panel"` containing a `lucide-sparkles` icon.
- [x] Update `apps/kanban-app/ui/src/components/ai-panel-container.test.tsx` collapsed-rail assertions: exactly one button with `aria-label="Expand AI panel"` whose icon is `lucide-sparkles`; no `lucide-panel-right-open` in the rail.
- [x] Run `pnpm --filter kanban-ui test -- ai-panel ai-panel-container` and confirm green.

## Workflow

- Use `/tdd` — flip the expectations in the two test files first, watch them fail, then update `ai-panel.tsx` and `ai-panel-container.tsx` to make them pass.