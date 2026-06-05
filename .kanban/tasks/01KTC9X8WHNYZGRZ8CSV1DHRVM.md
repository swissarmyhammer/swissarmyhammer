---
assignees:
- claude-code
depends_on:
- 01KTC9WJ2TT6GTTT6ZAXVX4XFN
position_column: todo
position_ordinal: '9180'
project: semantic-search
title: 'Kanban search real-pipeline e2e: semantic + exact-identifier + filter scoping'
---
## What
Add a real-pipeline end-to-end test for `search tasks` that proves the whole kanban-search feature works through the REAL path: real tasks created through the real kanban dispatch, embedded via the REAL `EmbeddingCache` + `Embedder`, ranked by the REAL `search tasks` op. NOT fixture-only — do NOT raw-insert vectors into the sidecar; let the op embed them. The embedder is always available (the Test runner has a GPU) — embed for real; there is NO model-free variant.

New test file: `crates/swissarmyhammer-kanban/tests/search_tasks_e2e.rs` (a `tests/` integration test; the crate already runs integration tests under `tests/`, e.g. `perspective_migration.rs`). Use the crate's `test-support` feature and a `TempDir` board. Gate it the same way the embedding crate gates model-dependent tests (serial / model-available convention; mirrors the code-context `semantic_search_e2e.rs` reference — runs on the GPU Test runner, skipped under the CPU-forced coverage gate).

Test shape:
- Create a real board via the real dispatch (`parse_input` + `execute_operation`, or `InitBoard`/`AddTask` builders) with several ordinary tasks, plus one task whose title contains a distinctive identifier (e.g. `"reticulate_splines refactor"`). No special "adversarial" tasks are needed.
- Run `search tasks` (through `execute_operation` so it goes via the registered op) with:
  1. A SEMANTIC query (a paraphrase, e.g. `"clean up the spline interpolation code"`) — assert a semantically-related task ranks highly, demonstrating the cosine signal contributes.
  2. An EXACT/typo identifier query (e.g. `"reticulate_splne"`) — assert the identifier task ranks `matches[0]`, and assert its `signals.bm25 > 0.0` OR `signals.trigram > 0.0`, demonstrating the lexical signal drove the rank (a typo embeds poorly, so cosine alone wouldn't surface it). Do NOT assert anything about "not being the top cosine" — that is flaky; the signals-non-zero + rank-1 checks are the reliable evidence here, and the rigorous fusion-necessity differential is owned by the code-search e2e card + the search-crate unit tests.
- DSL filter scoping: tag/scope a subset and run `search tasks` with a `filter` (e.g. `#bug`); assert ONLY in-scope tasks appear in results (out-of-scope tasks are excluded before ranking).
- Cache behavior: after the first `search tasks`, assert the sidecar `<root>/search-cache.sqlite3` exists and has rows; a second call reuses them (no re-embed) — assert via timing-independent means (row count stable / a seeded-cache spy), not wall-clock.
- Cold-start rebuild (simulates a fresh clone on another machine where the gitignored cache is absent): DELETE `<root>/search-cache.sqlite3` (and any `-wal`/`-shm`) after it has been populated, then run `search tasks` again and assert it transparently recreates the sidecar and returns the SAME correct ranking. Proves the cross-machine rebuild guarantee through the op (card 6 proves it at the store layer).

## Acceptance Criteria
- [ ] New e2e test exists at `crates/swissarmyhammer-kanban/tests/search_tasks_e2e.rs`, runs through the real kanban dispatch + real `search tasks` op + real embedder (no raw-inserted vectors).
- [ ] The exact/typo identifier query ranks the identifier task `matches[0]` with `signals.bm25` or `signals.trigram` non-zero (lexical signal proven through the real op).
- [ ] The semantic/paraphrase query ranks a semantically-related task highly (cosine signal proven).
- [ ] A DSL `filter` correctly scopes the corpus so out-of-scope tasks are absent from results.
- [ ] The sidecar cache is created and populated by the op; a second call reuses it (no re-embed).
- [ ] **Cold-start rebuild:** deleting the sidecar and re-running `search tasks` transparently recreates it and yields the same correct ranking.

## Tests
- [ ] `cargo test -p swissarmyhammer-kanban --test search_tasks_e2e` passes on a model-available runner (gated under the serial/model convention), including the filter-scoping, cache-reuse, and cold-start-rebuild assertions.

## Workflow
- Use `/tdd` — write the failing e2e first (it fails on trunk because `search tasks` does not exist), then rely on the implemented op + cache to make it pass.