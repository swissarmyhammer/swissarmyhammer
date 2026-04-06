---
assignees:
- claude-code
depends_on:
- 01KN5A6S38B42AZZ43AXCKT2KT
position_column: done
position_ordinal: fffffffffffffffff280
title: Update handle.rs undo/redo to use apply_patch instead of before/after text
---
Change undo() and redo() to apply patches. For concurrent edits, try apply_patch first; if it fails return MergeConflict. Drop three_way_merge usage.