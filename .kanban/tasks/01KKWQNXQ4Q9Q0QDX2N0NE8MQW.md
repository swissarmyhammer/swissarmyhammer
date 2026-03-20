---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffbb80
title: '[nit] FileDiff missing PartialEq derive'
---
avp-common/src/turn/diff.rs:10\n\nPer Rust review guidelines, new public types should implement applicable traits. `FileDiff` has `Debug, Clone, Serialize, Deserialize` but not `PartialEq` — useful for test assertions.\n\nAdd `PartialEq` to the derive list.\n\n**Verify**: `cargo test -p avp-common` passes after adding the derive. #review-finding