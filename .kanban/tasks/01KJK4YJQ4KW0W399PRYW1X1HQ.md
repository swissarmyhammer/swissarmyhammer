---
title: Update frontend types and add colored tag pills to detail panel and cards
position:
  column: done
  ordinal: b0
---
Update TypeScript types for the new tag model and render colored tag pills in TaskDetailPanel and TaskCard. No backward compat — clean break.

**Type updates (types/kanban.ts):**
- `Tag`: remove `name` field. Just `{ id, color, description }`.
- `Task.tags`: still `string[]` — these are tag names computed by the backend

**TaskDetailPanel (task-detail-panel.tsx):**
- Accept `tags: Tag[]` from board context
- Build `Map<string, Tag>` for lookups
- Render tags as colored pills using `color-mix(in srgb, #{tag.color} 15%, transparent)` background
- Show tag description on hover via `title` attribute

**TaskCard (task-card.tsx):**
- Accept `tags: Tag[]` from board context
- Show small colored dots or mini-pills for the task's tags below the title
- Look up colors from the tags map

**App.tsx / BoardView:**
- Thread `board.tags` down to TaskDetailPanel and TaskCard components

**Files:** `ui/src/types/kanban.ts`, `ui/src/components/task-detail-panel.tsx`, `ui/src/components/task-card.tsx`, `ui/src/App.tsx`, `ui/src/components/board-view.tsx`

- [ ] Update Tag/Task TypeScript interfaces
- [ ] Add colored tag pills to TaskDetailPanel
- [ ] Add tag dots/pills to TaskCard
- [ ] Thread board.tags through component tree
- [ ] npm run build passes
- [ ] npm run test passes