---
position_column: done
position_ordinal: e6
title: 'WARNING: read_changelog silently drops malformed JSONL lines'
---
**Resolution:** Already fixed. `read_changelog` already logs `tracing::warn!` with path, line number, and error for each unparseable line. The skip count return value was considered unnecessary since warnings provide the same info. Test added to confirm malformed lines are skipped while valid ones are preserved.\n\n- [x] Add tracing::warn for unparseable lines with line number and preview\n- [x] Consider returning `(Vec<ChangeEntry>, usize)` — not needed, tracing::warn suffices\n- [x] Add a test with a deliberately malformed line to verify the warning fires