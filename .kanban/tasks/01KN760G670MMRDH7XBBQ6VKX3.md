---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffff9c80
title: Test sem model identity match_entities edge cases (move/rename detection)
---
File: crates/swissarmyhammer-sem/src/model/identity.rs (71.0% coverage, 66/93 lines)\n\nUncovered code (~27 lines):\n- Move detection logic - entities with same content but different file paths (lines 166-182)\n- Rename detection - entities with same structural hash but different names (lines 191-219)\n- Similarity threshold boundary cases (lines 296-311)\n\nThe core matching (added/deleted/modified) is covered but cross-file move detection and rename detection are not. Create test cases with entities that moved between files or were renamed." #coverage-gap