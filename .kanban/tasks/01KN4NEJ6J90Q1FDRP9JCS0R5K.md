---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffb580
title: Fix attachments.yaml editor/display overrides
---
## What

Remove the incorrect `editor: multi-select` and `display: badge-list` overrides from `builtin/fields/definitions/attachments.yaml`. The `FieldType::Attachment` variant already infers the correct values (`editor: "attachment"`, `display: "attachment-list"` for multiple). The explicit overrides cause the frontend to use the wrong components (entity reference editors instead of file attachment UI).

### Files to modify
- `swissarmyhammer-kanban/builtin/fields/definitions/attachments.yaml` — remove `editor:` and `display:` lines

## Acceptance Criteria
- [ ] `attachments.yaml` has no explicit `editor` or `display` fields
- [ ] Field definition loads with inferred `editor: "attachment"` and `display: "attachment-list"`

## Tests
- [ ] Run: `cargo test -p swissarmyhammer-kanban` — all pass
- [ ] Run: `cargo test -p swissarmyhammer-fields` — all pass