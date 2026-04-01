---
assignees:
- claude-code
depends_on:
- 01KN4QPGVXP1DS6G7R5N3GJKXS
position_column: done
position_ordinal: ffffffffffffffffdf80
title: EntityTypeStore — TrackedStore impl with serialize/deserialize round-trip
---
## What

Implement `TrackedStore<Item = Entity, ItemId = EntityId>` via an `EntityTypeStore` struct parameterized by `EntityDef` + `FieldDef`s. Prove the format round-trips perfectly against existing on-disk files. No wiring into EntityContext or dispatch — just the trait impl and tests.

**Files to create:**
- `swissarmyhammer-entity/src/store.rs` — `EntityTypeStore` struct + `TrackedStore` impl

**Files to modify:**
- `swissarmyhammer-entity/Cargo.toml` — add dep on `swissarmyhammer-store`
- `swissarmyhammer-entity/src/lib.rs` — add `EntityId(String)` newtype, export store module

**Approach:**

### EntityId newtype
```rust
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityId(String);  // NOT Copy — slugs are strings
```

### EntityTypeStore
```rust
pub struct EntityTypeStore {
    root: PathBuf,
    entity_def: Arc<EntityDef>,
    field_defs: Arc<Vec<FieldDef>>,
}

impl TrackedStore for EntityTypeStore {
    type Item = Entity;
    type ItemId = EntityId;

    fn root(&self) -> &Path { &self.root }
    fn item_id(&self, entity: &Entity) -> EntityId { EntityId(entity.id.clone()) }
    fn extension(&self) -> &str {
        if self.entity_def.body_field.is_some() { "md" } else { "yaml" }
    }
    fn serialize(&self, entity: &Entity) -> Result<String> {
        // Strip computed fields (check field_defs for computed flag)
        // Apply field defaults for missing non-computed fields
        // Deterministic field ordering (sorted by key) for clean diffs
        // If body_field: MD+YAML frontmatter format
        // If no body_field: plain YAML
    }
    fn deserialize(&self, id: &EntityId, text: &str) -> Result<Entity> {
        // If body_field: parse frontmatter + body
        // If no body_field: parse plain YAML
        // Flatten nested objects one level deep
        // Inject entity_type and id from constructor/filename
    }
}
```

### Key serialize details
- Computed fields identified by checking `FieldDef` for a derive/compute marker
- Field ordering: sort frontmatter keys alphabetically for deterministic output
- Body field excluded from frontmatter, appended after `---` delimiter
- Empty body: still include the `---` delimiter with empty body after it

### Key deserialize details
- Split on `---` for MD+YAML format
- Parse YAML frontmatter via `serde_yaml_ng`
- Flatten nested objects: `{position: {column: "todo"}}` → `position_column: "todo"`
- Inject `entity_type` from `EntityDef.name` and `id` from filename

### Round-trip test strategy
- Create entities programmatically, serialize, deserialize, compare
- Read existing entity files from a test fixture, deserialize, re-serialize, compare text
- Test both MD+YAML (task with body) and plain YAML (column, tag) formats
- Test computed field stripping (field present in Entity but absent in serialized output)
- Test field defaults (missing field gets default value on deserialize)
- Test nested object flattening

## Acceptance Criteria
- [ ] `EntityId(String)` newtype exported
- [ ] `EntityTypeStore` implements `TrackedStore`
- [ ] Serialize matches existing on-disk format for tasks (MD+YAML)
- [ ] Serialize matches existing on-disk format for columns/tags/etc (plain YAML)
- [ ] Computed fields stripped during serialize
- [ ] Nested objects flattened during deserialize
- [ ] Deterministic field ordering in serialized output
- [ ] Round-trip: serialize → deserialize → serialize produces identical text

## Tests
- [ ] `cargo nextest run -p swissarmyhammer-entity` — new store tests + existing tests still pass