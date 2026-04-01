---
assignees:
- claude-code
depends_on:
- 01KN4QPGVXP1DS6G7R5N3GJKXS
position_column: todo
position_ordinal: '8780'
title: Migrate entity to TrackedStore — do/undo/redo, all tests green
---
## What

Implement `TrackedStore` for each entity type (task, column, swimlane, tag, actor, board). One store per entity type directory. Replace entity's own changelog, undo stack, and flush logic with the store crate. Wire `StoreContext` into the dispatch layer. **Stop here and manually test do/undo/redo before proceeding.**

**Key design:** One store per entity type, not one store for all entities. Each entity type has its own directory, serialization format (MD+YAML vs plain YAML), and ID format (ULID vs slug).

**Files to modify:**
- `swissarmyhammer-entity/Cargo.toml` — add dep on `swissarmyhammer-store`
- `swissarmyhammer-entity/src/lib.rs` — add `EntityId(String)` newtype, export it
- `swissarmyhammer-entity/src/stores.rs` — (new) `TrackedStore` impls for each entity type, driven by `EntityDef`

**Per entity type, a TrackedStore impl that knows:**
- `root()` → `.kanban/{type}s/` (e.g. `.kanban/tasks/`, `.kanban/columns/`)
- `item_id()` → `EntityId(String)` — ULID for tasks, slug for columns/tags/actors
- `serialize()` → MD+YAML if `EntityDef.body_field` is Some, plain YAML otherwise. Strips computed fields, applies defaults, deterministic field ordering for clean diffs.
- `deserialize()` → parse frontmatter+body or plain YAML, inject ID from filename, flatten nested objects one level

**EntityId newtype:**
```rust
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityId(String);  // NOT Copy — slugs are strings
// Display, FromStr — trivial delegation
```

**Approach:**

### Store construction from EntityDef
Rather than separate structs per type, one generic `EntityTypeStore` parameterized by `EntityDef`:
```rust
pub struct EntityTypeStore {
    root: PathBuf,
    entity_def: Arc<EntityDef>,
    field_defs: Arc<Vec<FieldDef>>,  // needed for computed field stripping + defaults
}

impl TrackedStore for EntityTypeStore {
    type Item = Entity;
    type ItemId = EntityId;
    // serialize/deserialize dispatch on entity_def.body_field
}
```

This means one struct, constructed per entity type with different `EntityDef`s. Register one `StoreHandle<EntityTypeStore>` per entity type in `StoreContext`.

### Migration path
- `EntityContext::write()` delegates to the appropriate `StoreHandle` (looked up by entity type)
- `EntityContext::delete()` delegates to `StoreHandle::delete()`
- `EntityContext` keeps: in-memory entity registry, schema, validation engine, computed field derivation
- Validation happens in `EntityContext` before calling `store.write()` — validation is above the store
- Computed field stripping happens in `EntityTypeStore::serialize()` — it has access to `FieldDef`s to know which fields are computed
- `push_undo_stack()` removed — `StoreHandle::write()` returns `UndoEntryId`, command layer pushes to `StoreContext`

### Dispatch layer changes
- `AppState` holds `StoreContext`
- On board open: register one `StoreHandle<EntityTypeStore>` per entity type
- After undoable commands: `store_context.flush_all()` replaces `flush_and_emit_for_handle()`
- `UndoCmd`/`RedoCmd` call `store_context.undo()`/`redo()`

### Changelog migration
- Existing per-entity `.jsonl` changelogs are NOT migrated — they stay for backward compatibility / activity history
- New store-level `changelog.jsonl` (one per entity type directory) is the undo source of truth going forward
- Old undo_stack.yaml entries from before the migration won't match new UndoEntryIds — the stack starts fresh on first run after migration (acceptable — better than a complex migration)

### Files to delete
- `swissarmyhammer-entity/src/undo_stack.rs` — moved to store crate
- `swissarmyhammer-entity/src/changelog.rs` — replaced by store crate (may keep for activity feed, but undo no longer uses it)

### Manual test checklist (do this before moving on)
1. Open existing board — all entities load correctly (no format changes)
2. Create a task — file on disk, new changelog.jsonl entry with before/after text
3. Edit a task field — changelog entry with text diff
4. Cmd+Z — edit is undone, file reverts to previous text
5. Cmd+Shift+Z — edit is redone
6. Delete a task → Cmd+Z restores it from trash
7. Edit a column name → Cmd+Z reverts (plain YAML entity)
8. External file edit → flush detects change, UI updates
9. Transaction: command that writes multiple entities → single Cmd+Z undoes all
10. All existing tests pass

## Acceptance Criteria
- [ ] `EntityId(String)` newtype exported from entity crate
- [ ] `EntityTypeStore` implements `TrackedStore<Item=Entity, ItemId=EntityId>`
- [ ] One `StoreHandle<EntityTypeStore>` registered per entity type
- [ ] Serialize matches existing on-disk format (MD+YAML / plain YAML)
- [ ] Computed fields stripped in serialize
- [ ] Deserialize handles both formats, injects ID from filename
- [ ] `EntityContext::write()` delegates to StoreHandle
- [ ] `EntityContext::delete()` delegates to StoreHandle
- [ ] `UndoCmd`/`RedoCmd` dispatch through StoreContext
- [ ] `dispatch_command_internal` uses `store_context.flush_all()`
- [ ] Existing boards load without migration
- [ ] Do/undo/redo works end-to-end (manually verified)
- [ ] All existing tests green

## Tests
- [ ] `cargo nextest run -E 'rdeps(swissarmyhammer-entity)'` — all existing entity tests pass
- [ ] `cargo nextest run -E 'rdeps(swissarmyhammer-kanban)'` — all kanban tests pass
- [ ] `cargo nextest run --workspace` — no regressions
- [ ] Manual: full checklist above