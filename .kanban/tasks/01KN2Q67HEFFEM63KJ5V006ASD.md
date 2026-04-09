---
assignees:
- claude-code
depends_on:
- 01KN2Q5EZYYNNZZAFQEFWYXMVQ
- 01KN2Q5Q5CTFGX2EHEYNANQNHN
position_column: done
position_ordinal: ffffffffffffffffaa80
title: 'PERSP-4: Perspective CRUD operations'
---
## What

Create the five perspective operations as Operation/Execute structs in `swissarmyhammer-kanban/src/perspective/`:

- `add.rs` — `AddPerspective`: Takes `name`, `view`, optional `fields`/`filter`/`group`/`sort`. Generates ULID. Writes via PerspectiveContext. Logs create to PerspectiveChangelog. Returns JSON.
- `get.rs` — `GetPerspective`: Takes `id` (ULID or name). Tries ID lookup, falls back to name. Read-only. Optionally resolves field ULIDs to field metadata when FieldsContext available.
- `list.rs` — `ListPerspectives`: No params. Returns all perspectives as JSON array. Read-only.
- `update.rs` — `UpdatePerspective`: Takes `id`, optional `name`/`view`/`fields`/`filter`/`group`/`sort`. Partial update — only provided fields change. Logs update (previous + current) to changelog.
- `delete.rs` — `DeletePerspective`: Takes `id`. Reads for changelog snapshot, deletes. Logs delete.

Each implements `Operation` + `Execute<KanbanContext, KanbanError>` following the tag/actor pattern. Mutations return `ExecutionResult::Logged`, reads return `ExecutionResult::Unlogged`.

## Acceptance Criteria
- [x] All five operations implement Operation + Execute traits
- [x] Add generates ULID, persists YAML, logs to changelog
- [x] Get works by ID and by name
- [x] List returns all perspectives with count
- [x] Update is partial (unchanged fields preserved)
- [x] Delete removes file and logs full snapshot
- [x] All mutations produce changelog entries

## Tests
- [x] `test_add_perspective` — basic creation
- [x] `test_add_perspective_minimal` — name + view only
- [x] `test_get_by_id` and `test_get_by_name`
- [x] `test_get_not_found` — returns error
- [x] `test_list_empty` and `test_list_multiple`
- [x] `test_update_partial` — only provided fields change
- [x] `test_delete_perspective`
- [x] `test_delete_not_found` — returns error
- [x] Run: `cargo test -p swissarmyhammer-kanban perspective`