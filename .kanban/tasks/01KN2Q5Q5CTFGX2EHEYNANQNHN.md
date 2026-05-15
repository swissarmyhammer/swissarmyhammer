---
assignees:
- claude-code
depends_on:
- 01KN2Q4ZRV7651J5XCQ4BXETKK
position_column: done
position_ordinal: ffffffffffffffffff8f80
title: 'PERSP-3: Perspective changelog (separate undo/redo)'
---
## What

Create `swissarmyhammer-kanban/src/perspective/changelog.rs` with a separate JSONL changelog for perspective mutations, stored at `.kanban/perspectives.jsonl`. Per the spec, perspective undo/redo is independent from entity undo.

**Types:**
- `PerspectiveChangeOp` enum: `Create`, `Update`, `Delete`
- `PerspectiveChangeEntry` struct: `id` (ULID), `timestamp`, `op`, `perspective_id`, `previous` (Option<serde_json::Value>), `current` (Option<serde_json::Value>)

**PerspectiveChangelog methods:**
- `new(path: PathBuf)` — path to `.kanban/perspectives.jsonl`
- `log_create(perspective: &Perspective)` — append create entry
- `log_update(id: &str, previous: &Perspective, current: &Perspective)` — append update entry with both snapshots
- `log_delete(perspective: &Perspective)` — append delete entry with full snapshot
- `read_all() -> Vec<PerspectiveChangeEntry>` — read all entries

Storage format: one JSON object per line, append-only.

**KanbanContext changes:**
- Initialize changelog path in context setup

## Acceptance Criteria
- [ ] Create/Update/Delete produce changelog entries
- [ ] Entries contain full perspective snapshots (previous + current)
- [ ] Append-only JSONL format
- [ ] `read_all()` returns entries in order
- [ ] Empty/nonexistent file returns empty vec

## Tests
- [ ] `changelog_log_create`
- [ ] `changelog_log_update_has_both_snapshots`
- [ ] `changelog_log_delete_has_previous`
- [ ] `changelog_read_multiple_entries`
- [ ] `changelog_empty_file_returns_empty`
- [ ] Run: `cargo test -p swissarmyhammer-kanban perspective::changelog`