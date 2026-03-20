---
position_column: done
position_ordinal: ffffff9180
title: Add embedding_enabled flag to IndexConfig
---
## What

Add `embedding_enabled: bool` to `IndexConfig` (default `true`). When `false`, `run_embedding_phase` skips entirely — no model load, no embedding, just parse + chunk + write to DB without embeddings.

This is the foundation for card 2 (making non-embedding tests fast) and card 3 (having a real embedding test that explicitly opts in).

### Files
- `swissarmyhammer-treesitter/src/index.rs`: Add field to `IndexConfig`, gate `run_embedding_phase`

## Acceptance Criteria
- [ ] `IndexConfig` has `embedding_enabled: bool`, default `true`
- [ ] When `embedding_enabled` is `false`, `scan_with_skip` completes without loading any model
- [ ] When `embedding_enabled` is `true`, behavior is unchanged from today
- [ ] Existing tests still pass (they all use default config which has `embedding_enabled: true`)

## Tests
- [ ] Unit test in `index.rs`: `test_config_embedding_disabled_skips_model` — create context with `embedding_enabled: false`, scan a dir with .rs files, assert `embedding_model` is `None` after scan completes and `files_embedded == 0`
- [ ] Run `cargo test -p swissarmyhammer-treesitter` — all existing tests pass