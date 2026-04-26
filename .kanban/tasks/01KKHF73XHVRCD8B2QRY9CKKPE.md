---
depends_on:
- 01KKHF6QWJ0N4CS7X9JBMR1FXA
position_column: done
position_ordinal: ffca80
title: 'SEM-1: Harvest model/ module (entity, change, identity)'
---
## What\nCopy the `model/` module from `vendor/sem/crates/sem-core/src/model/` into `crates/swissarmyhammer-sem/src/model/`. This is ~490 lines of pure Rust with no external deps beyond serde.\n\nFiles to copy:\n- `model/mod.rs` → `src/model/mod.rs`\n- `model/entity.rs` → `src/model/entity.rs` (SemanticEntity, build_entity_id)\n- `model/change.rs` → `src/model/change.rs` (ChangeType, SemanticChange)\n- `model/identity.rs` → `src/model/identity.rs` (match_entities, MatchResult, SimilarityFn)\n\nThese are pure data types + matching logic. Copy as-is, no modifications needed.\n\n## Acceptance Criteria\n- [ ] `swissarmyhammer_sem::model::entity::SemanticEntity` compiles\n- [ ] `swissarmyhammer_sem::model::change::{ChangeType, SemanticChange}` compiles\n- [ ] `swissarmyhammer_sem::model::identity::match_entities` compiles\n- [ ] All types are `pub` re-exported from `model/mod.rs`\n\n## Tests\n- [ ] `cargo check -p swissarmyhammer-sem` passes\n- [ ] Existing identity matching unit tests from sem-core are preserved and pass