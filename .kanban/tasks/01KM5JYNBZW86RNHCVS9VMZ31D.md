---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
title: Add mention_prefix and mention_display_field to task entity
---
## What
Add `mention_prefix: "^"` and `mention_display_field: title` to the task entity YAML definition. This is the foundation that enables both the dependencies editor autocomplete and inline body mentions.

**File:** `swissarmyhammer-kanban/builtin/fields/entities/task.yaml`

Add two lines:
```yaml
mention_prefix: "^"
mention_display_field: title
```

## Acceptance Criteria
- [ ] task.yaml has `mention_prefix: "^"` and `mention_display_field: title`
- [ ] Backend `search_mentions` returns task titles when called with `entity_type: "task"`
- [ ] `cargo nextest run` passes (no regressions)

## Tests
- [ ] Existing backend tests pass: `cargo nextest run`
- [ ] Manual: `search_mentions("task", "")` returns task entities with display_name = title