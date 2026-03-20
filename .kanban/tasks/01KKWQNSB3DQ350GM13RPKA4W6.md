---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffb980
title: '[nit] is_path_like vs is_likely_path confusingly similar names'
---
avp-common/src/turn/paths.rs:76,156\n\n`is_path_like` (new, stricter) and `is_likely_path` (old, laxer) do nearly the same thing with subtle differences. The names don't convey which is stricter.\n\nRename `is_path_like` to `is_path_structural` or add cross-referencing comments.\n\n**Verify**: check both functions have clear doc comments distinguishing them.