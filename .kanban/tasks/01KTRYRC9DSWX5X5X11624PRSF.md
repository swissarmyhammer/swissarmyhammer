---
assignees:
- claude-code
position_column: todo
position_ordinal: '9780'
project: op-token-diet
title: Add detail=slim|full to list tasks + list archived (slim default); get task stays full
---
## What
Add a `detail` parameter to the `list tasks` op (`crates/swissarmyhammer-kanban/src/task/list.rs`, `ListTasks`) AND the `list archived` op (`crates/swissarmyhammer-kanban/src/task/archive.rs`, `ListArchived`): `"slim"` (the DEFAULT) or `"full"`. Motivation: `list tasks` returns fully enriched task JSON today (full `description`, `attachments`, and soon the `comments` conversation log), so every board listing the agent makes carries every card's complete description and comment history — serious token bloat once agents log progress comments on every card they work. A listing is for orienting/selecting; the full payload belongs on the single-task fetch. `list archived` has the identical problem (it maps every archived task through full `task_entity_to_json`) and gets the identical treatment: same `slim_task_json` projection by default, same `detail` param semantics, same clear error on an unknown value.

- **slim** (default): an explicit ALLOWLIST projection of the enriched task JSON — roughly what the UI card renders: `id`, `short_id`, `title`, `position` (column + ordinal), `project`, `tags`, `filter_tags`/`virtual_tags`, `assignees`, `progress`, the dependency fields (`depends_on`, `blocked_by`, `blocks`), `ready`, and the date fields (`created`, `updated`, `due`, `scheduled`, `started`, `completed`). EXCLUDES `description`, `comments`, `attachments` — and, because it is an allowlist, any future heavy field is excluded by default.
- **full**: today's enriched shape, unchanged. `comments` rides into `full` automatically once the comments field exists (no dependency on the comments cards).
- **`get task` is NOT changed** — a single-task fetch always returns the full enriched task. An agent fetching one card to work it needs the description and conversation log anyway; a slim single-get has no real use case.
- An unrecognized `detail` value is a clear `KanbanError` (not a silent fallback) — on both ops.

Implementation:
- Add `pub detail: Option<String>` to the `ListTasks` op struct. It is OPTIONAL, so the wire `x-op-signatures` entry for `list tasks` is unchanged (required params only). Document the param so the FULL/CLI schema surface (`generate_kanban_mcp_schema_full` → `x-operation-schemas`) describes both values and the default; the schema-driven CLI then exposes `--detail` for free.
- `ListArchived` is currently a unit struct (`pub struct ListArchived;`) — give it the same `detail: Option<String>` field (builder-style like `ListTasks`), apply the projection in its `execute`, and have the `list archived` dispatch arm in `crates/swissarmyhammer-kanban/src/dispatch.rs` read the optional `detail` param the same way the `list tasks` arm does. Wire signature for `list archived` likewise unchanged; document `detail` in its full-schema entry too.
- Add a `pub(crate) fn slim_task_json(&Value) -> Value` allowlist projection next to `task_entity_to_rich_json` (in `task_helpers` / `task/shared`), applied per task in `ListTasks::execute` when `detail` is absent or `"slim"` — and reused as-is by `ListArchived::execute` (one projection, two call sites; never a second copy).
- Pagination, filtering, sorting, and `count`/`total` are unaffected — only the per-task shape changes.

Related (other project): the `search tasks` card in `semantic-search` maps hits back to enriched task JSON — a revision note on that card points it at this slim shape (+ `score`/`signals`).

NOTE: this changes the default agent/CLI-facing `list tasks` and `list archived` shapes. The desktop UI does not consume these ops (it reads through the entity store), so the UI is unaffected — but any existing test or skill prose that assumes `list tasks`/`list archived` returns `description` must be updated as part of this card.

## Acceptance Criteria
- [ ] `list tasks` with no `detail` returns slim tasks: allowlist fields present; NO `description`, `comments`, or `attachments` keys.
- [ ] `list archived` with no `detail` returns the same slim projection per archived task; `detail: "full"` returns the full `task_entity_to_json` shape.
- [ ] `list tasks` with `detail: "full"` returns today's enriched shape (description present).
- [ ] `detail: "slim"` is accepted explicitly; an unknown value errors clearly — on both ops.
- [ ] `get task` still returns the full enriched task (unchanged).
- [ ] The `detail` param does NOT appear in the wire `x-op-signatures` required lists for `list tasks` or `list archived`; it IS documented in both ops' full-schema `x-operation-schemas` entries.
- [ ] Existing tests/callers that assumed descriptions in list output are migrated (grep for `list tasks` and `list archived` assertions on `description`).
- [ ] `cargo clippy -p swissarmyhammer-kanban -- -D warnings` clean.

## Tests
- [ ] `slim_task_json` unit test: given an enriched task value with description/comments/attachments, the projection contains exactly the allowlist fields and none of the heavy ones.
- [ ] `list.rs` op tests (TempDir board pattern: `InitBoard` + `AddTask`): default list response tasks lack `description`; `detail:"full"` includes it; unknown `detail` errors; `get task` on the same board returns `description`.
- [ ] `archive.rs` op tests (same pattern + `ArchiveTask`): default `list archived` tasks lack `description`; `detail:"full"` includes it; unknown `detail` errors.
- [ ] Schema test: `list tasks` and `list archived` signatures in wire `x-op-signatures` unchanged (no `detail`).
- [ ] `cargo nextest run -p swissarmyhammer-kanban` — green.

## Workflow
- Use `/tdd` — write the projection + default-slim list tests first, then implement.