---
position_column: todo
position_ordinal: c9
title: 'Add `validate: Option<String>` to EntityDef'
---
Entity-level validation runs after all field-level validations pass. It receives the full validated entity and can enforce cross-field constraints (e.g., "if status is Done, due date must be set"). Same JS function body pattern as field-level validate.

## Struct change

In `swissarmyhammer-fields/src/types.rs`, add to `EntityDef`:
```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub validate: Option<String>,
```

Add `validate: None` to all EntityDef struct literals in:
- `types.rs` — `entity_def_yaml_round_trip` and `entity_def_without_body_field` tests
- `context.rs` — `write_and_read_entity`, `fields_for_entity_resolves`, `persistence_survives_reopen` tests

## ValidationEngine changes

In `swissarmyhammer-fields/src/validation.rs`, add a `validate_entity()` method to `ValidationEngine`:
- Receives `&EntityDef` and `&mut HashMap<String, Value>`
- If `entity_def.validate` is None, no-op
- Wraps the JS body in a function with `ctx = { entity: name, fields: {...} }`
- Executes via the existing JS runtime
- If the function returns an object, merges values back into fields
- On error, returns `FieldsError::ValidationFailed` with `field: "entity:<name>"`

## EntityContext wiring

In `swissarmyhammer-entity/src/context.rs`, update `apply_validation()`:
- After the field-validation loop (still inside the validation engine guard)
- Look up the entity def, call `engine.validate_entity(entity_def, &mut entity.fields)`
- Map errors to `EntityError::ValidationFailed`

Execution order: strip computed -> apply defaults -> validate fields -> validate entity -> persist.

## Tests

- EntityDef YAML round-trip with `validate: Some("...")`
- `validate_entity()` is no-op when validate is None
- `validate_entity()` runs JS and can transform fields
- `validate_entity()` can reject with error
- Integration test: entity def with validate, write entity, confirm validation ran

## Checklist

- [ ] Add `validate` field to EntityDef struct with serde annotations
- [ ] Add `validate: None` to all EntityDef struct literals in test code
- [ ] Add `validate_entity()` method to ValidationEngine
- [ ] Wire entity-level validation into EntityContext.apply_validation()
- [ ] Add unit tests for validate_entity (no-op, transform, reject)
- [ ] Add YAML round-trip test for EntityDef with validate
- [ ] Run `cargo test -p swissarmyhammer-fields -p swissarmyhammer-entity`