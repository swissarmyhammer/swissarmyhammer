---
assignees:
- claude-code
position_column: done
position_ordinal: '80'
title: Create swissarmyhammer-entity-search crate scaffold!
---
## What
Create a new workspace crate `swissarmyhammer-entity-search/` that provides in-memory search over `Entity` objects from `swissarmyhammer-entity`. Add to workspace members in root `Cargo.toml`.

The crate operates on `Entity` directly — it uses `entity.fields` (HashMap<String, Value>) and entity.get_str() to access searchable text. No raw markdown parsing needed; the entity layer handles that.

Files to create:
- `swissarmyhammer-entity-search/Cargo.toml` — deps: swissarmyhammer-entity, model-embedding, fuzzy-matcher, serde, serde_json, async-trait, tokio, tracing, thiserror
- `swissarmyhammer-entity-search/src/lib.rs` — public API re-exports
- `swissarmyhammer-entity-search/src/result.rs` — `SearchResult` (entity_id, score, strategy, matched_field), `SearchStrategy` enum (Fuzzy | Semantic)
- `swissarmyhammer-entity-search/src/error.rs` — `SearchError` type

## Acceptance Criteria
- [x] Crate compiles: `cargo check -p swissarmyhammer-entity-search`
- [x] `SearchResult` and `SearchStrategy` types are public
- [x] Depends on `swissarmyhammer-entity` for the `Entity` type

## Tests
- [ ] `cargo nextest run -p swissarmyhammer-entity-search`