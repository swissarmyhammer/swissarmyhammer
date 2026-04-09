---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffd680
title: Perspective.id is a bare String instead of PerspectiveId
---
swissarmyhammer-perspectives/src/types.rs\n\nThe `Perspective` struct uses `pub id: String` for its identifier, despite a `PerspectiveId` newtype being defined in the same crate (lib.rs via `define_id!`). This violates the type-safety guideline: two parameters of the same primitive type with different meanings must use newtypes.\n\nThe same issue applies to `PerspectiveFieldEntry.field: String` and `SortEntry.field: String` -- these hold field ULIDs but use bare Strings.\n\nSuggestion: Change `Perspective.id` to `PerspectiveId`. For the field ULID references, consider a `FieldDefId` newtype (which already exists in swissarmyhammer-fields). #review-finding