---
position_column: done
position_ordinal: e7
title: 'WARNING: text_diff_round_trip test only covers single-hunk substitution'
---
**Resolution:** All test cases already exist:\n- `diff_then_reverse_restores_original` — 20-line, multi-hunk insert+modify\n- `diff_then_reverse_with_scattered_edits` — 50-line, 4 scattered changes\n- `diff_then_reverse_with_deletions` — 30-line, deletions across hunks\n- `diff_then_reverse_mixed_insertions_and_deletions` — 20-line, insert+delete+substitute\n\nNo code changes needed.\n\n- [x] Add multi-hunk round-trip test\n- [x] Add test with net line insertion + substitution\n- [x] Add test with net line deletion + substitution\n- [x] Verify all round-trips