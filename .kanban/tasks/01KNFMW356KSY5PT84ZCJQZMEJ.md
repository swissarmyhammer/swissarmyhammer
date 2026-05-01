---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffff8480
title: define_id! macro generates Default that creates random ULID -- surprising semantics
---
swissarmyhammer-common/src/id_types.rs -- define_id! macro\n\nThe `Default` impl calls `Self::new()`, which generates a fresh ULID. This is semantically misleading -- `Default` typically implies a zero/empty value, not a random one. The `UndoEntryId` in swissarmyhammer-store explicitly suppresses this with `#[allow(clippy::new_without_default)]` and a comment explaining why.\n\nThe macro-generated types (PerspectiveId, EntityId, etc.) all silently generate random IDs on `Default::default()`. Code like `#[derive(Default)]` on a struct containing these IDs would silently create random identifiers.\n\nSuggestion: Remove the `Default` impl from the macro. If callers need a new random ID, they should call `::new()` explicitly. #review-finding