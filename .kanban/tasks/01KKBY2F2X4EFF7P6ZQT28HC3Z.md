---
position_column: done
position_ordinal: fd80
title: 'Bug: kanban update task does not persist depends_on'
---
## What

When calling `update task` with `depends_on: [\"<id>\"]`, the dependency is not persisted. The response shows `depends_on: []` and the task markdown frontmatter has no `depends_on` field.

`add task` with `depends_on` also appears to not persist — cards created with dependencies in this session all show empty arrays when read back.

## Repro

```json
{"op": "update task", "id": "01KKBWXW6VFCW78SG1WK6NC8GV", "depends_on": ["01KKBWXFZZXDVC7EFCXDTQ1YNE"]}
```

Response shows `depends_on: []`. Task file has no depends_on in frontmatter.

## Expected

`depends_on` should be written to the task's markdown frontmatter and returned in subsequent reads.

## Key Files

- `swissarmyhammer-kanban/` — task persistence logic