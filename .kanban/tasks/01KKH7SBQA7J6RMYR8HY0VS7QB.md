---
depends_on:
- 01KKH7RYD7CCTBN48W76ZF1E2M
position_column: done
position_ordinal: ffbf80
title: 'fix: stop silently swallowing mark_ts_indexed errors'
---
In `swissarmyhammer-code-context/src/indexing.rs` (line 127), `let _ = mark_ts_indexed(&db, &file_path);` silently discards errors. When `mark_ts_indexed` fails due to `SQLITE_BUSY`, the file stays dirty forever with no log output.

**Fix**: Replace `let _ =` with proper error logging (at minimum `eprintln!` or `tracing::warn!`). Ideally, failed files should be retried on the next worker loop iteration rather than silently dropped.

**Files**: `swissarmyhammer-code-context/src/indexing.rs`

#bug #code-context