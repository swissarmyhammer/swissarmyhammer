---
assignees:
- claude-code
depends_on:
- 01KN4S7XVZJ7ZZ6MSB7WAG2F99
position_column: done
position_ordinal: ffffffffffffffffe880
title: Extract validation layer above EntityContext::write()
---
## What

Extract the validation pipeline out of `EntityContext::write()` into a standalone layer that runs before `store.write()`. Validation is a domain concern, not a storage concern. After this card, `EntityContext::write()` still does its own file I/O — we're only extracting validation, not wiring StoreHandle yet.

**Files to modify:**
- `swissarmyhammer-entity/src/context.rs` — extract validation from `write()` into a separate `validate_entity()` method or standalone function
- `swissarmyhammer-entity/src/lib.rs` — export validation if it becomes a separate module

**What to extract:**
From the current `EntityContext::write()`:
1. `apply_validation()` — field-level validation via ValidationEngine
2. Cross-field entity-level validation
3. Computed field stripping (strip before write, derive on read)
4. Field default application

**After extraction:**
```rust
// Before: validation + write bundled
ctx.write(&entity).await?;

// After: validation is explicit, write is clean
let validated = ctx.validate_for_write(&entity)?;
ctx.write(&validated).await?;
```

The validate step:
- Strips computed fields
- Applies defaults
- Runs field-level validation
- Runs entity-level cross-field validation
- Returns the cleaned entity ready for storage

## Acceptance Criteria
- [ ] Validation logic extracted into a separate callable step
- [ ] `EntityContext::write()` still works (calls validation internally for now)
- [ ] A new `validate_for_write()` method is available for future callers
- [ ] All existing tests pass unchanged

## Tests
- [ ] `cargo nextest run -E 'rdeps(swissarmyhammer-entity)'` — no regressions