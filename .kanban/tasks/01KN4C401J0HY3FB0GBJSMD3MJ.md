---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffb080
title: Add Attachment variant to FieldType enum
---
## What

Add `Attachment` as a new variant of the `FieldType` enum in `swissarmyhammer-fields/src/types.rs`. This is the foundation — declaring that attachment is a first-class field kind alongside Text, Date, Number, etc.

### Variant shape
```rust
Attachment {
    /// Max file size in bytes. Defaults to GitHub's 100MB limit.
    #[serde(default = "default_max_bytes")]
    max_bytes: u64,
    /// Whether this field holds multiple attachments.
    #[serde(default)]
    multiple: bool,
}
```

### YAML definition
A field definition using it would look like:
```yaml
name: attachments
type:
  kind: attachment
  multiple: true
  max_bytes: 104857600  # 100MB, optional — this is the default
```

### Editor/display inference
- `editor`: `"attachment"` (new — frontend will need a file picker)
- `display`: `"attachment-list"` if multiple, `"attachment"` if single
- `sort`: `SortKind::Lexical` (sort by filename)

### Files to modify
- `swissarmyhammer-fields/src/types.rs` — add `Attachment` variant, update `default_editor()`, `default_display()`, `default_sort()`

## Acceptance Criteria
- [x] `FieldType::Attachment { max_bytes, multiple }` variant exists
- [x] YAML round-trip works: `kind: attachment` serializes and deserializes
- [x] `max_bytes` defaults to 100MB (104857600) when omitted
- [x] `multiple` defaults to false when omitted
- [x] Default editor/display/sort are inferred correctly

## Tests
- [x] Test: `FieldType::Attachment` YAML round-trip (like existing `field_type_*_yaml_round_trip` tests)
- [x] Test: default `max_bytes` is 100MB when not specified in YAML
- [x] Test: `default_editor()` returns `"attachment"` / `default_display()` returns correct value
- [x] Run: `cargo test -p swissarmyhammer-fields` — all pass