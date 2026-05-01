---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffff9280
title: 'NIT: Perspective struct uses String for id instead of a newtype'
---
swissarmyhammer-perspectives/src/types.rs:61-62\n\nThe Perspective struct uses `pub id: String` and `pub name: String`. Per the Rust review guidelines (newtypes for semantic distinctions), these two String fields have different semantics -- id is a ULID and name is a human-readable label -- but both are plain String. A caller could accidentally pass name where id is expected.\n\nThe same issue exists for PerspectiveFieldEntry::field (a ULID reference) and SortEntry::field.\n\nSuggestion: This is consistent with how the entity system uses String IDs elsewhere in the codebase, so enforcing newtypes only here would be inconsistent. This is a codebase-wide concern, not specific to this PR. No action needed on this branch.",
<parameter name="tags">["review-finding"] #review-finding