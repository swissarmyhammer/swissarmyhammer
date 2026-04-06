---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffff080
title: Change ChangelogEntry to store forward_patch/reverse_patch instead of before/after
---
Modify changelog.rs struct and tests. Replace Option<String> before/after with String forward_patch/reverse_patch.