---
position_column: done
position_ordinal: ffffffff8580
title: 'FAIL: test_find_all_duplicates_detects_near_identical_functions - cluster includes unrelated.rs'
---
Test in swissarmyhammer-treesitter/tests/workspace_leader_reader.rs:544 panics with: "Cluster should NOT contain unrelated.rs (different semantics)". The duplicate detection is incorrectly clustering an unrelated function with semantically different functions.