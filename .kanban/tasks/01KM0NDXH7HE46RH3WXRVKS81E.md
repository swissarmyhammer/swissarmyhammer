---
assignees:
- claude-code
depends_on:
- 01KM0C2NDHKEB7QXG4XEAQ0KR5
position_column: done
position_ordinal: '7e80'
title: Replace DOM snapshot with icon-based OS ghost + DOM draggable in target window
---
## What
The dom-to-image-more card snapshot looks terrible. Replace with a three-tier approach:

**Tier 1 — Source window (intra-window drag):**
Already works — @dnd-kit DragOverlay renders a real EntityCard component. No change needed.

**Tier 2 — Between windows (OS drag ghost):**
Use a clean, small icon-based image for `drag::Image::Raw()`. NOT a DOM screenshot. Options:
- Embed a pre-made PNG icon (e.g. a stylized card/document icon, maybe 48x48 or 64x64)
- Or generate a simple canvas-drawn rectangle with the task title text
- The OS ghost is intentionally minimal — it just needs to signal "you're dragging something"

**Tier 3 — Target window (cross-window drop):**
When the drag enters a target window, render a real DOM draggable using the task data from the drag session (`session.task_fields`). The `CrossWindowDropOverlay` already has access to the session — it should render an `EntityCard`-like component that follows the cursor position from `DragDropEvent.position`.

**Files to change:**
- `kanban-app/ui/src/lib/drag-session-context.tsx` — remove `captureElementAsPng` usage, pass null for preview image (or a static icon)
- `kanban-app/ui/src/lib/capture-element.ts` — DELETE this file
- `kanban-app/ui/src/types/dom-to-image-more.d.ts` — DELETE this file
- `kanban-app/ui/package.json` — remove `dom-to-image-more` dependency
- `kanban-app/ui/src/components/cross-window-drop-overlay.tsx` — render EntityCard from session.task_fields at DragDropEvent position
- `kanban-app/src/commands.rs` — embed a static PNG icon as the default drag image, or simplify to always use the 1x1 placeholder (the OS ghost is decorative, not the primary feedback)

## Acceptance Criteria
- [ ] Intra-window drag shows the real EntityCard (DragOverlay, unchanged)
- [ ] OS drag ghost between windows shows a clean icon (not a broken DOM screenshot)
- [ ] Target window shows a rendered EntityCard component following the cursor
- [ ] dom-to-image-more dependency is removed
- [ ] No visible quality regression in any drag scenario

## Tests
- [ ] Manual test: drag within window — looks the same as before
- [ ] Manual test: drag between windows — OS ghost is a clean icon
- [ ] Manual test: hover over target window — full card appears at cursor
- [ ] `npm run build` — no warnings
- [ ] `cargo nextest run` — no regressions