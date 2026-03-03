---
title: Make Task.tags computed from description markdown
position:
  column: done
  ordinal: a7
---
The core behavioral change: `tags` on Task becomes computed by parsing `#tag` patterns from the description. The stored `tags` array is simply removed — no backward compat needed.

**Task struct changes (types/task.rs):**
- Remove `pub tags: Vec<TagId>` field entirely
- Add computed method: `pub fn tags(&self) -> Vec<String>` that calls `tag_parser::parse_tags(&self.description)` and returns tag names
- Remove `with_tags()` builder method

**Operation changes:**
- `task/add.rs`: Remove `tags` field from AddTask params
- `task/update.rs`: Remove `tags: Option<Vec<TagId>>` from UpdateTask params
- `task/list.rs`: Tag filter uses `task.tags()` computed method
- `board/get.rs`: Tag counting uses `task.tags()` computed method
- `task/tag.rs` and `task/untag.rs`: Temporarily stub to compile (rewritten in next card)

**Serialization:**
- Task JSON no longer has `tags` key on disk
- API responses (GetTask, ListTasks) include computed `tags` in output JSON

**Files:** `types/task.rs`, `task/{add,update,list,tag,untag}.rs`, `board/get.rs`, integration tests

- [ ] Remove tags field from Task struct
- [ ] Add computed tags() method using tag_parser
- [ ] Remove tags from AddTask and UpdateTask params
- [ ] Update ListTasks tag filter to use computed tags
- [ ] Update GetBoard tag counting
- [ ] Ensure API responses include computed tags
- [ ] Fix all tests
- [ ] cargo test passes