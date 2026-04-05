---
assignees:
- claude-code
position_column: todo
position_ordinal: b180
title: Add `groupable` attribute to FieldDef (Rust + YAML definitions)
---
## What

Add an opt-in `groupable` boolean to field definitions so the UI can filter which fields appear in the group-by selector. This card covers the backend: Rust struct and YAML field definition files. Also fix the stale Rust doc comment that says `group` is a "JS expression" — it's a plain field name.

### Files to modify

- `swissarmyhammer-fields/src/types.rs` — add `pub groupable: Option<bool>` to `FieldDef` struct (line ~136) with `#[serde(default, skip_serializing_if = "Option::is_none")]`
- `swissarmyhammer-perspectives/src/types.rs` — update doc comment on `Perspective.group` from "Opaque group function string (JS expression). Stored, not evaluated." to "Group-by field name. Stored as a plain field name string, consumed by the UI as a TanStack Table grouping column ID."
- YAML field definitions in `swissarmyhammer-kanban/builtin/fields/definitions/` — add `groupable: true` to categorical fields:
  - `tags.yaml`
  - `assignees.yaml`
  - `color.yaml`
  - `tag_name.yaml`
- Leave all other fields without `groupable` (defaults to `None`/false — title, body, description, position_column, position_swimlane, position_ordinal, attachments, progress, etc.)

### Design decision

Opt-in (`groupable: true` required) rather than opt-out. This is safer — new fields are ungroupable by default, and only fields explicitly marked appear in the selector. Position fields (column, swimlane) are NOT groupable — those represent board layout, not meaningful categorical data for grid grouping.

## Acceptance Criteria

- [ ] `FieldDef` Rust struct has `groupable: Option<bool>` that round-trips through serde YAML and JSON
- [ ] `get_entity_schema` Tauri command serializes `groupable` in the JSON response
- [ ] The 4 categorical field YAML files have `groupable: true`
- [ ] position_column, position_swimlane, and all other field YAML files do NOT have `groupable`
- [ ] Rust doc comment on `Perspective.group` reflects reality (plain field name, not JS expression)
- [ ] Existing tests pass — no deserialization breakage from the new optional field

## Tests

- [ ] Add unit test in `swissarmyhammer-fields/src/types.rs` (or nearby test module): deserialize a YAML snippet with `groupable: true`, assert the field is `Some(true)`. Deserialize without it, assert `None`.
- [ ] Run: `cargo test -p swissarmyhammer-fields` — all tests pass
- [ ] Run: `cargo test -p swissarmyhammer-kanban` — all tests pass (entity loading with new field)

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.