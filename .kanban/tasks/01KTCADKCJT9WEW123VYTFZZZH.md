---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffa180
project: card-comments
title: Add CommentLog structured field type to swissarmyhammer-fields
---
## What
Define a new `CommentLog` variant on the `FieldType` enum in `crates/swissarmyhammer-fields/src/types.rs`. This is the data model + field metadata for a conversation log: an inline list of dependent comment members, each capturing **who** (actor id), **what** (free text), and **when** (ISO 8601 timestamp). Unlike `FieldType::Attachment` (which models separate `attachment` entities), comments are NOT their own entity kind — the value is stored inline on the owning task field as a JSON array of `{actor, text, timestamp}` objects.

Approach, mirroring the existing `Attachment` variant precedent in the same file:
- Add `CommentLog {}` (no params needed initially; it is always a multiple/list type) to the `#[serde(tag = "kind", rename_all = "kebab-case")]` `FieldType` enum. The serialized kind will be `comment-log`.
- Extend `FieldDef::effective_editor()` → return `"comment-log"` for `FieldType::CommentLog`.
- Extend `FieldDef::effective_display()` → return `"comment-log"` for `FieldType::CommentLog`.
- `effective_sort()` falls through to the existing `_ => SortKind::Lexical` default — no change needed, but add a test asserting it.
- The editor/display strings are plain frontend-resolved names (see the comment block in types.rs: "adding a new display type is a frontend-only change"); no Rust enum for them.

Do NOT add a separate `comment` EntityDef or comment entity YAML — comments are inline field values, not entities. The `attachment` entity precedent is deliberately NOT followed here.

## Acceptance Criteria
- [ ] `FieldType::CommentLog` exists and serializes to/from YAML with `kind: comment-log` (round-trip).
- [ ] `effective_editor()` returns `"comment-log"` and `effective_display()` returns `"comment-log"` for the new variant.
- [ ] All existing `FieldType` exhaustive `match` arms in the crate still compile (the enum addition forces the compiler to flag any non-exhaustive match — fix any in this crate).
- [ ] `cargo build -p swissarmyhammer-fields` and `cargo clippy -p swissarmyhammer-fields -- -D warnings` are clean.

## Tests
- [ ] In `crates/swissarmyhammer-fields/src/types.rs` test module, add `field_type_comment_log_yaml_round_trip` modeled on the existing `field_type_attachment_yaml_round_trip` test: serialize a `FieldDef` with `type: { kind: comment-log }`, deserialize, assert equality.
- [ ] Add `comment_log_field_infers_editor_display` (modeled on `attachment_field_infers_editor_display`) asserting `effective_editor()=="comment-log"` and `effective_display()=="comment-log"`.
- [ ] `cargo nextest run -p swissarmyhammer-fields` — all green.

## Workflow
- Use `/tdd` — write the failing round-trip + inference tests first, then add the enum variant and match arms to make them pass.