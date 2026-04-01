---
assignees:
- claude-code
depends_on:
- 01KN5A6S38B42AZZ43AXCKT2KT
position_column: done
position_ordinal: fffffffffffffffff380
title: Update all tests in handle.rs and changelog.rs for new patch-based ChangelogEntry
---
Fix test helpers and assertions. The three-way merge tests need rethinking since we now use apply_patch which either succeeds or fails.