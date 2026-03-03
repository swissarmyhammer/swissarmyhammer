---
title: Implement FieldsContext with YAML persistence
position:
  column: done
  ordinal: c7
---
Implement `FieldsContext` in `src/context.rs` — the main API surface for `swissarmyhammer-fields`. It manages the `fields/` directory, reads/writes field definitions and entity templates as YAML files.

**Directory structure managed:**
```
fields/
  definitions/    ← one .yaml per field (e.g. status.yaml)
  entities/       ← one .yaml per entity type (e.g. task.yaml)
  lib/            ← JS modules for validation (created but not managed here)
```

**API:**
```rust
pub struct FieldsContext { root: PathBuf, /* in-memory cache */ }

impl FieldsContext {
    // Construction (async — reads from disk)
    pub async fn open(root: impl Into<PathBuf>) -> Result<Self>;
    
    // Field definitions
    pub fn get_field_by_name(&self, name: &str) -> Option<&FieldDef>;
    pub fn get_field_by_id(&self, id: &Ulid) -> Option<&FieldDef>;
    pub fn all_fields(&self) -> &[FieldDef];
    pub async fn write_field(&mut self, def: &FieldDef) -> Result<()>;
    pub async fn delete_field(&mut self, id: &Ulid) -> Result<()>;
    
    // Entity templates
    pub fn get_entity(&self, name: &str) -> Option<&EntityDef>;
    pub fn all_entities(&self) -> &[EntityDef];
    pub async fn write_entity(&mut self, def: &EntityDef) -> Result<()>;
    
    // Lookup helpers
    pub fn fields_for_entity(&self, entity_name: &str) -> Vec<&FieldDef>;
    pub fn resolve_name_to_id(&self, name: &str) -> Option<Ulid>;
}
```

**Key behaviors:**
- `open()` creates directories if missing, then reads all .yaml files into memory
- Indexed by both ULID and name for fast lookup
- Write operations persist to YAML immediately (atomic write via temp file)
- File naming: definitions use field name as filename (e.g. `status.yaml`), entities use entity name

**Subtasks:**
- [ ] Implement directory creation in open()
- [ ] Implement YAML read for definitions/ directory
- [ ] Implement YAML read for entities/ directory
- [ ] Build in-memory indexes (by name and by ULID)
- [ ] Implement write_field with atomic YAML write
- [ ] Implement write_entity with atomic YAML write
- [ ] Implement delete_field (removes file + index entry)
- [ ] Implement lookup helpers (fields_for_entity, resolve_name_to_id)
- [ ] Write integration tests with tempdir