---
assignees:
- claude-code
position_column: todo
position_ordinal: 9c80
title: 'Bug: pasting a tag onto a task causes computed auto-tags to vanish'
---
## What

When cutting a tag from one task and pasting it onto another, the **computed tags** (the `tags` badge-list derived from `#tag` patterns in the body via `parse-body-tags`) vanish from the target task's UI. Copy/paste of tags by themselves works — the pasted tag appears — but the pre-existing auto-tags disappear.

### Reproduction

1. Create task A with body containing `#bug #feature`
2. Create task B with a `#urgent` tag
3. Cut `#urgent` from task B (Ctrl+X on the tag pill)
4. Paste onto task A (Ctrl+V with task A focused)
5. Expected: task A shows tags `bug`, `feature`, `urgent`
6. Actual: only `urgent` (or nothing) appears — existing auto-tags vanish

### Root cause investigation

The paste flow is: `PasteCmd` → `PasteTag::execute` (`swissarmyhammer-kanban/src/tag/paste.rs:46-122`). It reads the task, appends `#slug` to the body via `tag_parser::append_tag`, writes the entity. The write strips computed fields (`validate_for_write` at `swissarmyhammer-entity/src/context.rs:699-703`). After write, `enrich_computed_fields` (`kanban-app/src/commands.rs:1581-1649`) re-reads the entity with compute and appends computed fields to the `entity-field-changed` event.

**Likely failure point:** The enrichment chain. Investigate whether `enrich_computed_fields` correctly re-derives the `tags` field from the updated body. Possible issues:

1. **Stale read in enrichment** — `ectx.read()` in `enrich_computed_fields` might read from a stale cache or old file before the write has flushed. The `EntityContext::read` always reads from disk (`swissarmyhammer-entity/src/context.rs:170-178`), so this should be fresh, but the file watcher and store notification timing could interfere.

2. **Event dedup dropping the entity-field-changed** — If the watcher hash comparison at `kanban-app/src/commands.rs:1522-1529` determines "no diff" (because it's comparing pre-computed hashes), the event could get dropped as `"store item-changed but watcher saw no diff"`.

3. **Frontend patch clobber** — The `entity-field-changed` handler in `rust-engine-container.tsx:347-356` patches fields from `changes`. If the changes array includes `body` but NOT `tags`, the existing `tags` field in the store remains but might become stale (derived from old body). Verify the changes array includes `{field: "tags", value: [...]}`.

### Files to investigate

- `kanban-app/src/commands.rs:1581-1649` — `enrich_computed_fields` — add debug logging, verify `tags` is appended
- `kanban-app/src/commands.rs:1460-1548` — event building, verify the `EntityFieldChanged` event is produced (not deduped)
- `swissarmyhammer-kanban/src/tag/paste.rs:110-115` — verify body is correctly updated
- `kanban-app/ui/src/components/rust-engine-container.tsx:323-375` — verify frontend patch includes tags

## Acceptance Criteria

- [ ] After pasting a tag onto a task with existing auto-tags, ALL tags (old + new) remain visible in the tags badge-list
- [ ] The `entity-field-changed` event for the task includes the re-derived `tags` computed field in the changes array
- [ ] Add a test in `swissarmyhammer-kanban/src/tag/paste.rs` that verifies the task's `tags` field contains both old and new tags after paste

## Tests

- [ ] `swissarmyhammer-kanban/src/tag/paste.rs` — Add test: paste a tag onto a task that already has `#existing` in its body → verify `task_tags(&task)` returns both `existing` and the pasted tag
- [ ] `kanban-app/src/commands.rs` — Add integration test or logging: after `PasteTag` dispatch, verify the emitted `entity-field-changed` event's changes array includes a `tags` entry with the complete tag list
- [ ] `kanban-app/ui/src/lib/entity-event-propagation.test.tsx` — Add test: entity-field-changed with body+tags changes correctly patches both fields
- [ ] Run `cargo test -p swissarmyhammer-kanban` — all tests pass
- [ ] Run `cd kanban-app/ui && npx vitest run` — all tests pass

## Workflow

- Start by adding the backend test in `paste.rs` to reproduce the bug at the data layer
- If data layer is correct, add logging to `enrich_computed_fields` and test via manual paste
- Use `/tdd` for any code changes needed to fix #paste-tag-bug"
</invoke> #paste-tag-bug