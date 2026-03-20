---
depends_on:
- 01KKH7RYD7CCTBN48W76ZF1E2M
position_column: done
position_ordinal: ffc080
title: 'fix: delete old chunks before re-inserting in write_ts_chunks'
---
`write_ts_chunks` in `swissarmyhammer-code-context/src/indexing.rs` (line 228) does a plain `INSERT INTO ts_chunks` without first deleting existing chunks for the file. When a file is re-indexed (marked dirty after a hash change), duplicate chunk rows accumulate as garbage.

**Fix**: Add `DELETE FROM ts_chunks WHERE file_path = ?` before the INSERT loop in `write_ts_chunks`.

**Files**: `swissarmyhammer-code-context/src/indexing.rs`