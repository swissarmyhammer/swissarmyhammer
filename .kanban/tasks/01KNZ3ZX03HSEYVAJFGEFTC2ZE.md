---
assignees:
- wballard
position_column: done
position_ordinal: ffffffffffffffffffffffb880
project: pill-via-cm6
title: Make column entity mentionable (% prefix)
---
## What

Make `column` a first-class mentionable entity so `position_column` and any future column reference can flow through the same CM6 pill pipeline as project/task/actor/tag, eliminating the sole legitimate user of `BadgeDisplay`'s reference branch.

**Files to modify:**
- `swissarmyhammer-kanban/builtin/entities/column.yaml` — add:
  - `mention_prefix: "%"`
  - `mention_display_field: name`
  - `mention_color_field: color` (if column has a color field; otherwise omit)

Check whether `column` has a `color` field; if not, the decorator will fall back to the default gray, same as today.

**Downstream effect:** `useSchema().mentionableTypes` will now include column with prefix `%`. `useMentionExtensions()` will automatically wire up decoration/autocomplete/tooltip infrastructure for columns. No frontend changes needed in this card — this is schema-only.

**Out of scope:** Users typing `%col-name` in prose descriptions and having it auto-decorate is a natural consequence and is fine. No need to gate it.

## Acceptance Criteria
- [ ] `column.yaml` declares `mention_prefix: "%"` and `mention_display_field: name`
- [ ] `useSchema()` exposes column in `mentionableTypes` array (verify in test)
- [ ] `useMentionExtensions()` produces a non-empty extension array when called in a context where columns exist
- [ ] No behavioral regression in existing tests for projects/tasks/actors/tags

## Tests
- [ ] Update `kanban-app/ui/src/lib/schema-context.test.tsx` — add a test case that feeds a column entity YAML with the new fields and asserts `mentionableTypes` contains `{ entityType: "column", prefix: "%", displayField: "name" }`
- [ ] Add a case to `kanban-app/ui/src/hooks/__tests__/use-mention-extensions.test.ts` — provide a column in the entity store and assert the returned extensions include column decoration infra (one more extension than before)
- [ ] Run: `bun test schema-context.test.tsx use-mention-extensions.test.ts` — all pass

## Workflow
- Use `/tdd` — write the schema-context test first, watch it fail, then update `column.yaml` to make it pass. Then add the use-mention-extensions test. Then verify nothing else breaks.
