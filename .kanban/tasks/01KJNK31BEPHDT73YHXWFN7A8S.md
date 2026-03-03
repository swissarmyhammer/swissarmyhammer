---
title: Implement validation engine with JS execution
position:
  column: done
  ordinal: d0
---
Add a validation module to `swissarmyhammer-fields` that runs JavaScript validation functions using `swissarmyhammer-js`.

**EntityLookup trait:**
```rust
#[async_trait]
pub trait EntityLookup: Send + Sync {
    async fn get(&self, entity_type: &str, id: &str) -> Option<serde_json::Value>;
    async fn list(&self, entity_type: &str) -> Vec<serde_json::Value>;
}
```

**ValidationEngine:**
```rust
pub struct ValidationEngine {
    js: JsState,
    lookup: Option<Box<dyn EntityLookup>>,
}
impl ValidationEngine {
    pub fn new() -> Self;
    pub fn with_lookup(self, lookup: impl EntityLookup + 'static) -> Self;
    pub async fn validate(&self, field: &FieldDef, value: serde_json::Value, sibling_fields: &HashMap<String, serde_json::Value>) -> Result<serde_json::Value>;
}
```

**Validation runs on both read and write — clean in, clean out.**

**ctx object shape:**
```javascript
{
  value: <incoming value>,
  fields: { status: "In Progress", ... },
  name: "tag_name",
  lookup: async (type, id?) => entity|null or [entities]
}
```
- `ctx.lookup(type, id)` → entity object or null (get one)
- `ctx.lookup(type)` → array of all entities of that type

**Default reference validation** (auto for `kind: reference` without explicit validate):
Prune dangling IDs — `ctx.lookup(entityType, id) !== null`. Silent removal, no error.

**Subtasks:**
- [ ] Define EntityLookup trait with get(type, id) and list(type)
- [ ] Implement ValidationEngine with single lookup provider
- [ ] Implement ctx object construction (value, fields, name, lookup)
- [ ] Inject lookup as async callable into JS context
- [ ] Implement default reference validation (prune dangling IDs)
- [ ] Handle sync and async validation functions
- [ ] Add validate_field() convenience to FieldsContext
- [ ] Write test: tag_name validation
- [ ] Write test: reference validation prunes dangling IDs
- [ ] Write test: ctx.lookup(type) returns all entities
- [ ] Write test: no validate + non-reference passes through unchanged