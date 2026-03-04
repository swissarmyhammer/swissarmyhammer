---
position_column: todo
position_ordinal: c7
title: 'Add `width: Option<u32>` to FieldDef'
---
The spec defines `width` as the default column width in pixels on every field definition. This is missing from the code entirely. Perspectives can override it, but the field provides the default.

## Struct change

In `swissarmyhammer-fields/src/types.rs`, add after `display`:
```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub width: Option<u32>,
```

## Struct literal updates

Add `width: None` (or a specific value in round-trip tests) to every FieldDef struct literal in:
- `types.rs` — 7 test functions
- `context.rs` — 2 test helpers
- `compute.rs` — 2 test helpers
- `validation.rs` — 1 test helper
- `defaults.rs` — 3 test struct literals

Update `field_def_yaml_round_trip` test to use `width: Some(120)` and verify round-trip.

## Builtin YAML definitions

Add `width:` to all 21 files in `swissarmyhammer-kanban/builtin/fields/definitions/`:

| Field | Width | Field | Width |
|-------|-------|-------|-------|
| title | 300 | order | 80 |
| body | 400 | tag_name | 200 |
| tags | 200 | actor_type | 120 |
| assignees | 150 | progress | 100 |
| due | 120 | position_column | 120 |
| depends_on | 200 | position_ordinal | 100 |
| name | 200 | position_swimlane | 120 |
| color | 80 | attachments | 200 |
| description | 300 | attachment_name | 200 |
| | | attachment_path | 300 |
| | | attachment_mime_type | 120 |
| | | attachment_size | 100 |

## Checklist

- [ ] Add `width` field to FieldDef struct with serde annotations
- [ ] Add `width: None` to all FieldDef struct literals in test code
- [ ] Update round-trip test to exercise `width: Some(120)`
- [ ] Add width values to all 21 builtin YAML definition files
- [ ] Run `cargo test -p swissarmyhammer-fields -p swissarmyhammer-kanban`