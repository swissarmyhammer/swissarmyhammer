---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffa480
title: Extend ComputeEngine with EntityQueryFn for aggregate derives
---
## What
The `ComputeEngine` (`swissarmyhammer-fields/src/compute.rs`) currently only passes `&HashMap<String, Value>` (the entity's own fields) to `DeriveFn`. Aggregate computed fields like board `percent_complete` need to query other entities (tasks, columns).

Add an optional query callback so aggregate derives can read related entities:

1. Define a query trait/type in `swissarmyhammer-fields/src/compute.rs`:
   ```rust
   /// Read-only query interface for aggregate computed fields.
   pub type EntityQueryFn = Box<
       dyn Fn(&str) -> Pin<Box<dyn Future<Output = Vec<HashMap<String, serde_json::Value>>> + Send>>
           + Send + Sync,
   >;
   ```
   This takes an entity type string and returns a list of field maps — mirrors `EntityContext::list()` but returns raw field maps to avoid depending on swissarmyhammer-entity.

2. Add `derive_all_with_query()` method that accepts `Option<&EntityQueryFn>` and passes it through to `DeriveFn`. Add an alternate `DeriveFn` type or extend the existing one with an optional second parameter.

   Simplest approach: add a new `AggregateFn` type and a parallel `aggregations` map on `ComputeEngine`, or extend `derive_all` to accept an optional query fn and thread it through.

3. In `EntityContext::apply_compute()` (`swissarmyhammer-entity/src/context.rs:1057`), construct the query fn from `&self` (closure over the entity context) and pass it to `derive_all`.

### Files
- `swissarmyhammer-fields/src/compute.rs` — add query type, extend `derive_all`
- `swissarmyhammer-entity/src/context.rs` — pass query fn from `apply_compute`

## Acceptance Criteria
- [ ] `ComputeEngine::derive_all` accepts an optional entity query function
- [ ] Existing per-field derives (parse-body-tags, parse-body-progress) still work unchanged
- [ ] New aggregate derives can list entities of any type via the query fn
- [ ] No circular dependency between fields and entity crates (query fn is a closure, not a type dependency)

## Tests
- [ ] Existing `compute.rs` tests pass unchanged
- [ ] Add test: aggregate derive that sums a field across queried entities
- [ ] `cargo nextest run -p swissarmyhammer-fields` passes
- [ ] `cargo nextest run -p swissarmyhammer-entity` passes