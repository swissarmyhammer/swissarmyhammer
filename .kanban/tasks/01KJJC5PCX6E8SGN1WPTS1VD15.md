---
title: Update skills to use markdown checklists instead of subtask operations
position:
  column: done
  ordinal: b3
---
Update all builtin skill definitions that reference subtask operations to instead guide users/agents to put checklists in description markdown.

**Files to modify:**
- `builtin/skills/plan/SKILL.md` — Replace guidance about `add subtask` with putting `- [ ]` checklists in the task description field
- `builtin/skills/kanban/SKILL.md` — Remove subtask workflow (complete subtask, etc.), add guidance to track progress via markdown checklists in description
- `builtin/skills/implement/SKILL.md` — Remove any subtask references, update workflow to use markdown checklists

## Checklist
- [ ] Update plan skill — replace subtask ops with description checklists
- [ ] Update kanban skill — remove subtask workflow
- [ ] Update implement skill — remove subtask references
- [ ] Check for any other skills referencing subtasks