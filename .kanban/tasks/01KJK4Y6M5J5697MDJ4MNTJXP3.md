---
title: Add RenameTag operation
position:
  column: done
  ordinal: a9
---
New kernel operation: bulk find-replace `#oldname` → `#newname` across all card bodies. Atomic rename.

**New file: tag/rename.rs**
- `RenameTag` operation struct: `{ old_name: String, new_name: String }`
- Implementation:
  1. Verify old tag exists
  2. Verify new tag name doesn't already exist
  3. Iterate all tasks, call `tag_parser::rename_tag(description, old, new)` on each
  4. Rewrite any tasks whose description changed
  5. Create new tag file with old tag's color and description
  6. Delete old tag file
  7. Log the operation

**Registration:**
- Add to `tag/mod.rs`
- Add to MCP dispatch in `swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs`
- Add to KANBAN_OPERATIONS array
- Update op enum count in tests

**Integration tests:**
- Rename tag across multiple tasks
- Verify all descriptions updated
- Verify old tag file gone, new tag file has same metadata
- Error cases: rename to existing tag, rename non-existent tag

**Files:** `tag/rename.rs` (new), `tag/mod.rs`, `swissarmyhammer-tools/.../kanban/mod.rs`, integration tests

- [ ] Create RenameTag operation
- [ ] Register in tag/mod.rs
- [ ] Register in MCP dispatch
- [ ] Write integration tests
- [ ] cargo test passes