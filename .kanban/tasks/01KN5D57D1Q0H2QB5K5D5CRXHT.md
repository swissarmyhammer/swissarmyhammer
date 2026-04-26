---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffd280
title: Test store changelog.rs compact and corrupt line handling (76.9%)
---
**File**: `swissarmyhammer-store/src/changelog.rs` (76.9% -- 30/39 lines)\n\n**What**: Uncovered lines:\n- L71: `create_dir_all` in `append()` when parent dir doesn't exist\n- L77, L79, L81: `append()` internals -- file open, serde serialization, write_all\n- L92: `read_all()` -- Err(e) non-NotFound error path\n- L101-102: `read_all()` -- the corrupt line warning/skip branch\n- L119: `find_entry()` -- non-NotFound error path\n- L125: `find_entry()` -- blank line skip in reverse iteration\n\n**Acceptance criteria**: Coverage above 85% for changelog.rs\n\n**Tests to add**:\n- Test `read_all()` with a file containing an invalid JSON line (triggers corrupt line skip at L101-102)\n- Test `find_entry()` with blank lines interspersed (triggers L125)\n- The IO error paths (L92, L119) require permission-denied or similar IO errors which are harder to test portably" #coverage-gap