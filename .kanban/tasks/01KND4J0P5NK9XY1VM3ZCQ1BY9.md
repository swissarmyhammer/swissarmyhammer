---
assignees:
- claude-code
depends_on:
- 01KNESQHQ68QVG341SMAKD8DXZ
position_column: done
position_ordinal: ffffffffffffffffd880
title: 'VT-1: Add `virtual_tags` and `filter_tags` hidden field definitions on task entity'
---
## What

Two new hidden fields on the task entity. No display, no editor — pure data fields populated by the enrichment pipeline.

### 1. `virtual_tags` — hidden computed field
Computed virtual tag slugs set by enrichment. No UI display (backend-only for now).

### 2. `filter_tags` — hidden union field
`tags ∪ virtual_tags`. Used for backend filtering (`list tasks --tag`, `next task --tag`).

**Files to create:**
- `swissarmyhammer-kanban/builtin/fields/definitions/virtual_tags.yaml`:
  ```yaml
  name: virtual_tags
  description: Computed virtual tags based on task state
  type:
    kind: computed
    entity: tag
    commit_display_names: true
  icon: zap
  display: none
  editor: none
  ```
- `swissarmyhammer-kanban/builtin/fields/definitions/filter_tags.yaml`:
  ```yaml
  name: filter_tags
  description: Union of tags and virtual_tags for filtering
  type:
    kind: computed
    entity: tag
    commit_display_names: true
  display: none
  editor: none
  ```

**Files to modify:**
- `swissarmyhammer-kanban/builtin/fields/entities/task.yaml` — add `virtual_tags` and `filter_tags` to fields list

No tag entity changes. No frontend changes.

## Acceptance Criteria
- [ ] `virtual_tags` field definition exists with `display: none`, `editor: none`
- [ ] `filter_tags` field definition exists with `display: none`, `editor: none`
- [ ] Task entity schema includes both new fields
- [ ] Tag entity schema is unchanged
- [ ] Existing tests pass with no regression

## Tests
- [ ] `cargo nextest run -p swissarmyhammer-kanban` passes
- [ ] `pnpm --filter kanban-app-ui test` passes (new hidden fields don't break UI)

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#virtual-tags