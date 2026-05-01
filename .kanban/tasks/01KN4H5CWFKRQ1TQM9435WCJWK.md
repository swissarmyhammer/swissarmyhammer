---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffdc80
title: Add tests for PerspectiveContext::delete swap_remove index fixup
---
File: swissarmyhammer-perspectives/src/context.rs:132-163\n\nCoverage: 72.6% (53/73 lines in context.rs)\n\nUncovered lines: 146, 156, 157, 158, 159\n\nThe delete method uses swap_remove to maintain a compact Vec. When the deleted element is not the last one, the element swapped into its position needs its index entries updated (lines 156-159). Also, the IO error branch at line 146 (non-NotFound error during file removal) is uncovered.\n\nWhat to test:\n1. Delete a perspective that is NOT the last element in the Vec (triggers swap_remove index fixup). Verify that the remaining perspectives are still findable by both ID and name after the delete.\n2. Mock or induce an IO error on remove_file that is not NotFound (e.g., permission denied) and verify it propagates as PerspectiveError::Io. #coverage-gap