---
assignees:
- claude-code
depends_on:
- 01KNM6AEYM2DRRQ7CQ4DHXCX8N
position_column: done
position_ordinal: ffffffffffffffffffffffbb80
title: Inject _changelog into entity fields for changelog-dependent computed fields
---
## What

Extend the entity enrichment pipeline so that computed fields with `depends_on: ["_changelog"]` can access the entity's JSONL changelog entries during derivation.

**Approach:** In `EntityContext::apply_compute_with_query()` (`swissarmyhammer-entity/src/context.rs`), before calling `engine.derive_all()`:

1. Scan the entity's computed field defs for any that have `_changelog` in their `depends_on` list
2. If found, read the entity's changelog via `self.read_changelog(entity_type, entity_id)`
3. Serialize the `Vec<ChangeEntry>` to JSON and inject as `entity.fields["_changelog"]`
4. Call `derive_all()` as normal — derivation functions read `fields["_changelog"]`
5. After derivation, strip `_changelog` from entity.fields so it's not persisted or returned to callers

This uses the existing `depends_on` mechanism for signaling — no new function types, no new traits. The changelog read is lazy: only happens when at least one computed field needs it.

**Files to modify:**
- `swissarmyhammer-entity/src/context.rs` — `apply_compute_with_query()` method

**Performance note:** For `list()`, this means one JSONL file read per entity that has changelog-dependent fields. For large boards this could be slow. Acceptable for now; optimization (caching, summary files) is a follow-up concern.

## Acceptance Criteria
- [x] Computed fields with `depends_on: ["_changelog"]` receive changelog data in `fields["_changelog"]`
- [x] Computed fields without `_changelog` dependency are unaffected
- [x] `_changelog` key is stripped from entity fields after derivation
- [x] Entities with no changelog (new/empty) get `_changelog: []`
- [x] No changelog read when no computed fields need it

## Tests
- [x] Unit test: entity with changelog-dependent computed field → verify `_changelog` available during derivation
- [x] Unit test: entity with only non-changelog computed fields → verify no changelog read (no `_changelog` key injected)
- [x] Unit test: `_changelog` is stripped after derivation (not visible in returned entity)
- [x] `cargo test -p swissarmyhammer-entity` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement.

#task-dates