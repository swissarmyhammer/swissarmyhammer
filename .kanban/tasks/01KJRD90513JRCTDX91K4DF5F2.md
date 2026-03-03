---
position_column: done
position_ordinal: g1
title: 'Dead code: Task struct methods duplicated by task_helpers.rs'
---
**Done.** Removed all dead code from Task struct and related types.\n\nRemoved from Task: tags(), progress(), parse_checklist_counts(), is_ready(), blocked_by(), blocks(), find_comment(), find_comment_mut(), comments field, and 6 associated tests.\nRemoved types: Comment struct, CommentId.\nAlready absent: find_attachment, legacy migration methods.\n\n220 tests pass, clippy clean, full workspace compiles.