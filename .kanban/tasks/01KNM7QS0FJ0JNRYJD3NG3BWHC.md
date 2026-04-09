---
assignees:
- claude-code
depends_on:
- 01KNM7Q14EBEW0M6ZHBVTNHGQ2
position_column: todo
position_ordinal: '9680'
title: Register date derivation functions in kanban_compute_engine()
---
## What

Register four derivation functions in `kanban_compute_engine()` (`swissarmyhammer-kanban/src/defaults.rs`) that read from `fields[\"_changelog\"]` to derive system dates.

Each function reads the injected `_changelog` JSON array (which contains serialized `ChangeEntry` objects) and extracts the relevant timestamp.

**derive-created**: Timestamp of the first changelog entry (op: \"create\"). Falls back to first entry regardless of op. Returns ISO 8601 string or null.

**derive-updated**: Timestamp of the last changelog entry. Returns ISO 8601 string or null.

**derive-started**: Scan changelog for the first entry where `position_column` changed to a non-first column (i.e., work began). Look for `FieldChange::Changed` or `FieldChange::TextDiff` on `position_column` where the new value is not the first column (e.g., not \"todo\"). Returns ISO 8601 string or null if never started.

**derive-completed**: Scan changelog (reverse) for the last entry where `position_column` changed to the terminal column. Look for changes to `position_column` where new value matches terminal column. If the task was later moved out of done, return null. Returns ISO 8601 string or null.

**Determining first/terminal columns**: The derive functions receive entity fields only (no column metadata). Options:
- Convention: first column = \"todo\", terminal = \"done\" (fragile)
- Better: use an aggregate derivation (`register_aggregate`) that queries column entities to determine ordering, then scans changelog

Use `register_aggregate` for started/completed so they can query columns. Use simple `register` for created/updated (no column knowledge needed).

**Files to modify:**
- `swissarmyhammer-kanban/src/defaults.rs` — register four new derivations

## Acceptance Criteria
- [ ] `derive-created` returns timestamp of first changelog entry
- [ ] `derive-updated` returns timestamp of last changelog entry
- [ ] `derive-started` returns timestamp of first move to non-first column, null if never moved
- [ ] `derive-completed` returns timestamp of last move to terminal column, null if not in done or never completed
- [ ] All four derivations handle empty changelog gracefully (return null)
- [ ] `all_builtin_computed_fields_have_registered_derivations` test passes

## Tests
- [ ] Test `derive-created` with mock changelog entries → returns first entry timestamp
- [ ] Test `derive-updated` with mock changelog → returns last entry timestamp
- [ ] Test `derive-started` with changelog showing todo→doing transition → returns doing entry timestamp
- [ ] Test `derive-started` with task never moved → returns null
- [ ] Test `derive-completed` with changelog showing doing→done → returns done entry timestamp
- [ ] Test `derive-completed` with task moved done→doing (reopened) → returns null
- [ ] Test `derive-completed` with bounce: doing→done→doing→done → returns last done timestamp
- [ ] `cargo test -p swissarmyhammer-kanban` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement.

#task-dates