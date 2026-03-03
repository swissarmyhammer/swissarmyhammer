---
title: Rewrite TagTask/UntagTask as text mutations with auto-creation
position:
  column: done
  ordinal: a8
---
Rewrite TagTask to append `#tag` to description text (and auto-create Tag object if missing). Rewrite UntagTask to remove `#tag` from description text. Update DeleteTag cascade to use text removal.

**TagTask rewrite (task/tag.rs):**
- Read task
- Check if `#tag` already in description via parser — if yes, no-op
- Call `tag_parser::append_tag(&task.description, &tag_name)` for new description
- Write task
- **Auto-create**: if no Tag file exists for this tag name, create one with `auto_color(tag_name)` and empty description
- Return success JSON

**UntagTask rewrite (task/untag.rs):**
- Read task
- Call `tag_parser::remove_tag(&task.description, &tag_name)` for new description
- If description changed, write task
- Do NOT delete the Tag object (that's a separate DeleteTag operation)
- Return success JSON

**DeleteTag cascade update (tag/delete.rs):**
- Instead of removing tag IDs from task.tags array, iterate all tasks and call `tag_parser::remove_tag()` on each description
- Rewrite any tasks whose description changed
- Delete the tag file

**New integration tests:**
- Tag a task via TagTask → verify `#tag` in description
- Verify Tag object auto-created if it didn't exist
- Untag a task → verify `#tag` removed from description
- Tag idempotency: tagging twice doesn't duplicate
- DeleteTag cascades text removal across all tasks

**Files:** `task/tag.rs`, `task/untag.rs`, `tag/delete.rs`, `tests/integration_tag_storage.rs`

- [ ] Rewrite TagTask: append #tag to description
- [ ] Implement auto-creation of Tag objects
- [ ] Rewrite UntagTask: remove #tag from description
- [ ] Update DeleteTag cascade to use text removal
- [ ] Write integration tests for all paths
- [ ] cargo test passes