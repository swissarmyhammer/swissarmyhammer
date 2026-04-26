---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffaa80
title: Test store file change → ChangeEvent → entity enrichment pipeline
---
## What

Add integration tests proving that when a store-managed entity file changes on disk (external write), the `StoreHandle` cache detects the diff, `StoreContext::flush_all()` produces the correct `ChangeEvent`, and `flush_and_emit_for_handle()` converts it into enriched `WatchEvent`s that the entity layer populates with current field data.

The pipeline under test:

1. `StoreHandle::flush_changes()` (`swissarmyhammer-store/src/handle.rs:662`) — scans directory, compares against in-memory cache, produces `ChangeEvent` with `event_name` = `item-created` / `item-changed` / `item-removed`
2. `StoreContext::flush_all()` (`swissarmyhammer-store/src/context.rs:165`) — aggregates events from all registered stores
3. `flush_and_emit_for_handle()` (`kanban-app/src/commands.rs:1409`) — converts `ChangeEvent` → `WatchEvent`, calls `EntityContext::read()` to populate `fields` map

### Existing test coverage (gaps)
- `swissarmyhammer-store/src/handle.rs:957-994` — tests `flush_changes` for external create/change/remove with `MockStore` (plain strings). Does NOT test with `EntityTypeStore` or verify the `ChangeEvent` payload contains correct `store_name`.
- `swissarmyhammer-store/src/context.rs:287-307` — tests `flush_all` aggregates events from two stores. Does NOT verify event payloads.
- No test exercises the full path from `EntityTypeStore` file change → `ChangeEvent` → entity enrichment.

### Files to modify
- `swissarmyhammer-store/src/handle.rs` — add test: `flush_changes` produces events with correct `store_name` in payload when using a named store
- `swissarmyhammer-entity/src/store.rs` or new `swissarmyhammer-entity/tests/store_change_detection.rs` — add integration test: write entity YAML externally → `StoreHandle<EntityTypeStore>::flush_changes()` returns `item-changed` with correct entity type name and ID
- `swissarmyhammer-store/src/context.rs` — add test: `flush_all` event payloads contain correct `store` and `id` fields

## Acceptance Criteria
- [ ] Test: externally create a YAML entity file → `StoreHandle<EntityTypeStore>::flush_changes()` returns `item-created` event with `store` = entity type name and `id` = file stem
- [ ] Test: externally modify an existing YAML entity file → `flush_changes()` returns `item-changed` event with correct payload
- [ ] Test: externally delete an entity file → `flush_changes()` returns `item-removed` event with correct payload
- [ ] Test: `flush_all()` event payloads contain `store` and `id` fields matching the store name and item ID (not just count check)
- [ ] Test: entity enrichment — after `item-changed`, calling `EntityContext::read()` returns updated field values (proving the entity layer sees the new disk state)

## Tests
- [ ] `cargo test -p swissarmyhammer-store handle::tests::flush_changes_event_payload_includes_store_name` — new test
- [ ] `cargo test -p swissarmyhammer-store context::tests::flush_all_event_payloads_have_store_and_id` — new test
- [ ] `cargo test -p swissarmyhammer-entity store_change_detection` or inline test — new test exercising `EntityTypeStore` + `StoreHandle` + external file write → correct `ChangeEvent`
- [ ] `cargo test -p swissarmyhammer-store` — all existing tests pass
- [ ] `cargo test -p swissarmyhammer-entity` — all existing tests pass