---
title: Implement with_defaults() seeding mechanism
position:
  column: done
  ordinal: c8
---
Add the `with_defaults()` builder pattern to `FieldsContext` and the `FieldDefaults` builder type.

**FieldDefaults builder:**
```rust
pub struct FieldDefaults {
    fields: Vec<FieldDef>,
    entities: Vec<EntityDef>,
}
impl FieldDefaults {
    pub fn new() -> Self;
    pub fn field(mut self, def: FieldDef) -> Self;
    pub fn entity(mut self, def: EntityDef) -> Self;
}
```

**Builder integration:**
```rust
impl FieldsContext {
    pub fn open(root: impl Into<PathBuf>) -> FieldsContextBuilder;
}
impl FieldsContextBuilder {
    pub fn with_defaults(mut self, defaults: FieldDefaults) -> Self;
    pub async fn build(self) -> Result<FieldsContext>;
}
```

**Seeding behavior (ULID-matched):**
- For each default field: if no file with that ULID exists, write it. If exists (even if renamed), skip.
- Entity templates: match by name, write if absent, skip if present.
- First open writes all. Subsequent opens only add new defaults. User customizations preserved.

**Subtasks:**
- [ ] Implement FieldDefaults builder struct
- [ ] Implement FieldsContextBuilder with with_defaults()
- [ ] Implement ULID-matched seeding logic for fields
- [ ] Implement name-matched seeding logic for entity templates
- [ ] Write tests: first open seeds all defaults
- [ ] Write tests: subsequent open skips existing definitions
- [ ] Write tests: user-modified definitions are preserved