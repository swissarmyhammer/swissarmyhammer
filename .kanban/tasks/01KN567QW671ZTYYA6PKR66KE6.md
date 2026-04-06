---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffee80
title: 'Fix StoreHandle: per-item changelogs instead of per-store changelog'
---
## What

The StoreHandle currently writes a single `{root}/changelog.jsonl` per store. This is wrong. Each item needs its own changelog file alongside it: `{root}/{item_id}.jsonl` next to `{root}/{item_id}.yaml`.

This is critical because:
1. Changelog travels with the entity to trash/archive (both files move together)
2. Self-contained history per item
3. Scoped lookup — find_entry only searches one item's changelog

## Changes needed

### StoreHandle (swissarmyhammer-store/src/handle.rs)
- Remove the single `changelog: Changelog` field
- Instead, derive changelog path per item: `item_path.with_extension("jsonl")`
- `write()`: append to `{root}/{item_id}.jsonl`
- `delete()`: trash both the item file AND its changelog (move both to `.trash/`)
- `undo()` of delete: restore both files from trash
- `undo()` of create: trash both files
- `find_entry()`: search only the specific item's changelog

### Changelog (swissarmyhammer-store/src/changelog.rs)
- `Changelog::new(path)` still works — it's already per-file
- Remove any assumption of one changelog per store
- `find_entry` already works on a single file — no change needed

### Trash (swissarmyhammer-store/src/trash.rs)
- `trash_file` needs to also move the `.jsonl` changelog alongside the data file
- `restore_file` needs to also restore the `.jsonl` changelog
- Or: separate calls for data file and changelog

### StoreContext
- `has_entry()` needs to know which item to search. The undo stack entry needs to include the item_id so we know which changelog to search.
- Update `UndoEntry` to include `item_id: String` alongside `id` and `label`

### ChangelogEntry
- Add `item_id` field if not already present (it is — good)

### Tests
- Update all handle tests to verify per-item changelogs
- Verify trash moves both data + changelog
- Verify undo/redo finds entries in the correct per-item changelog

## Acceptance Criteria
- [ ] Each item write creates/appends to `{item_id}.jsonl` next to the item file
- [ ] No `{root}/changelog.jsonl` file created
- [ ] Trash moves both data file and changelog
- [ ] Restore brings back both files
- [ ] find_entry searches the correct per-item changelog
- [ ] All 65+ store tests pass (updated)
- [ ] Workspace builds clean

## Tests
- [ ] `cargo nextest run -p swissarmyhammer-store` — all pass