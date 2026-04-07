---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: '9880'
title: Backfill system dates from existing JSONL changelogs
---
## What

Existing tasks have no date fields but their JSONL changelogs contain all the data needed to derive them. Add a migration/backfill that reads each task's `.jsonl` and sets:

- **created** — timestamp of the first changelog entry (op: create)
- **updated** — timestamp of the last changelog entry
- **started** — timestamp of the first `position_column` change to a non-first column (e.g., `doing`)
- **completed** — timestamp of the last `position_column` change to terminal column (e.g., `done`)

**Implementation approach:**

Add a function in `swissarmyhammer-kanban` (e.g., `task::backfill_dates`) that:
1. Lists all task entities
2. For each task, reads its `.jsonl` changelog
3. Scans entries for create timestamp, last update timestamp, and column transitions
4. Sets the appropriate date fields on the entity
5. Writes the entity (which will also set `updated` to now — acceptable for migration)

This could be exposed as a CLI subcommand (`sah kanban backfill-dates`) or run automatically on first load after the feature lands.

**Recommendation:** Run once as a CLI command. Don't auto-migrate on every startup — it's a one-time operation.

**Files to create/modify:**
- `swissarmyhammer-kanban/src/task/backfill.rs` — backfill logic
- `swissarmyhammer-kanban/src/task/mod.rs` — register module
- Wire into CLI or MCP as a one-shot command

## Acceptance Criteria
- [ ] Backfill reads JSONL and correctly derives created, started, completed dates
- [ ] Tasks that were never moved to doing have no `started` date
- [ ] Tasks in done column have `completed` set to when they were moved there
- [ ] Tasks not in done have no `completed` date
- [ ] Running backfill is idempotent — safe to run multiple times

## Tests
- [ ] Integration test: create task with known JSONL history → run backfill → verify dates match JSONL timestamps
- [ ] Integration test: task with no column moves → verify only created/updated set
- [ ] Integration test: task that bounced between columns → verify started is first doing entry, completed is last done entry
- [ ] `cargo test -p swissarmyhammer-kanban` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement.

#task-dates