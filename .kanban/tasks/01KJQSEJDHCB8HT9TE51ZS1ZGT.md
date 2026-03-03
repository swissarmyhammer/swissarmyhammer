---
position_column: done
position_ordinal: e5
title: 'WARNING: apply_unified_diff silently uses diff context over actual file content'
---
`/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/swissarmyhammer-entity/src/changelog.rs` lines 306-310\n\n**Resolution:** Bug does not exist. The hand-rolled `apply_unified_diff` was replaced by `apply_changes` which delegates to `diffy::apply`. The `diffy` library validates context lines against actual content and returns an error on mismatch. Test added to confirm.\n\n- [x] Add context line validation or use old_lines content instead of diff content\n- [x] Add a test that detects stale diff application\n- [x] Decide on error behavior: return `Result<String>` with an error, or log a warning #warning