---
title: 'Migrate existing task data: convert subtasks to markdown checklists'
position:
  column: done
  ordinal: b4
---
Write a one-time migration that converts any existing tasks with structured subtasks into markdown checklists in the description.

For each task with non-empty `subtasks` array:
1. Append a checklist to the description: `- [x] title` for completed, `- [ ] title` for incomplete
2. Clear the subtasks array (or let serde skip handle it)

This can be a standalone script or integrated into the serde compat layer. Since the current board has 0 tasks with subtasks, this is mostly a safety net for other boards.

## Checklist
- [ ] Write migration logic (iterate tasks, convert subtasks to markdown)
- [ ] Handle edge cases (empty description, already has checklists)
- [ ] Test with sample data
- [ ] Run on actual board data