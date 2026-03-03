---
title: Update frontend to remove subtask UI
position:
  column: done
  ordinal: b2
---
Remove structured subtask rendering from the frontend. Keep the progress badge but source it from markdown checklist parsing (the backend will compute progress from description).

**Files to modify:**
- `swissarmyhammer-kanban-app/ui/src/types/kanban.ts` — Remove `Subtask` interface, remove `subtasks` field from `Task` interface
- `swissarmyhammer-kanban-app/ui/src/components/task-card.tsx` — Update progress badge to use the computed `progress` field from the backend instead of counting subtasks client-side
- `swissarmyhammer-kanban-app/ui/src/components/task-detail-panel.tsx` — Remove the subtask checklist section entirely (checklists will be visible in the description text)

## Checklist
- [ ] Remove Subtask interface from kanban.ts
- [ ] Remove subtasks from Task interface
- [ ] Update progress badge in task-card.tsx to use backend progress field
- [ ] Remove subtask section from task-detail-panel.tsx
- [ ] Verify app builds and renders correctly