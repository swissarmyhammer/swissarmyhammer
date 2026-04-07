---
assignees:
- claude-code
depends_on:
- 01KNM6AEYM2DRRQ7CQ4DHXCX8N
position_column: todo
position_ordinal: '9580'
title: Inject Created and Updated system dates in EntityContext::write()
---
## What

Modify `EntityContext::write()` in `swissarmyhammer-entity/src/context.rs` to automatically set `created` and `updated` timestamps on task entities.

**Created** — set once during create (when `previous.is_none()`). Value is `Utc::now().to_rfc3339()`. Never overwritten on subsequent writes.

**Updated** — set to `Utc::now().to_rfc3339()` on every write that produces changes. Always reflects the most recent mutation.

**Implementation approach:**

In `EntityContext::write()` (around line 194, after `validate_for_write` but before the store handle write):

1. Read previous entity (already done at line 200)
2. If `previous.is_none()` (create): set `created` field on the entity
3. Always set `updated` field on the entity
4. These fields will then be included in the diff and changelog automatically

**Important:** Only inject dates for entity types that have these fields defined. Check `def.fields` for `created`/`updated` before setting them, so non-task entities aren't affected.

**Files to modify:**
- `swissarmyhammer-entity/src/context.rs` — inject dates in `write()`

## Acceptance Criteria
- [ ] New tasks get `created` field set to current UTC timestamp on first write
- [ ] `created` is never overwritten on subsequent writes
- [ ] `updated` is set to current UTC timestamp on every write that changes something
- [ ] Non-task entities (columns, tags, actors) are unaffected
- [ ] Changelog entries include the date field changes

## Tests
- [ ] Unit test: create entity → verify `created` and `updated` are set
- [ ] Unit test: update entity → verify `created` unchanged, `updated` refreshed
- [ ] Unit test: entity type without date fields → no injection
- [ ] `cargo test -p swissarmyhammer-entity` passes
- [ ] `cargo test -p swissarmyhammer-kanban` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement.

#task-dates