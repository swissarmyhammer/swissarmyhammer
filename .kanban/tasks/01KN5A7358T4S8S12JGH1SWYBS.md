---
assignees:
- claude-code
depends_on:
- 01KN5A6S38B42AZZ43AXCKT2KT
position_column: done
position_ordinal: ffffffffffffffffffffc980
title: Update handle.rs write/delete to compute and store patches
---
Change write() and delete() to call create_patches() and store forward_patch/reverse_patch instead of before/after.