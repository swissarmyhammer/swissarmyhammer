---
depends_on:
- 01KKFEK574ADD49DRZNNTA22BJ
position_column: done
position_ordinal: ffffffa680
title: Write a real end-to-end duplicate detection test
---
## What

Replace the vacuous `test_leader_can_query_duplicates` / `test_find_all_duplicates_with_files` with a test that **actually proves** the embedding + duplicate detection pipeline works. The current tests assert `is_ok()` on empty results — they prove nothing.

### Key constraints from the code
- `cluster_by_similarity` only clusters chunks from **different files** (line 942 of unified.rs)
- Chunks are only created for `EMBEDDABLE_NODE_KINDS` — in Rust, that means `function_item`, `impl_item`, `struct_item`, etc.
- The test must create files with genuinely duplicate/near-duplicate function bodies across files
- `min_chunk_bytes` filters out tiny chunks, so functions need real bodies

### Test design
Create a temp workspace with 3 files:
- `utils_a.rs`: contains `fn process_data(items: &[i32]) -> Vec<i32> { items.iter().filter(|x| **x > 0).map(|x| x * 2).collect() }`
- `utils_b.rs`: contains a near-copy with trivial renames: `fn transform_data(values: &[i32]) -> Vec<i32> { values.iter().filter(|v| **v > 0).map(|v| v * 2).collect() }`
- `unrelated.rs`: contains a completely different function (string processing, not numeric)

Open workspace with `embedding_enabled: true`, wait for indexing, call `find_all_duplicates`. Assert:
1. At least one `DuplicateCluster` is returned
2. The cluster contains chunks from both `utils_a.rs` and `utils_b.rs`
3. `unrelated.rs` is NOT in the same cluster
4. The cluster's `avg_similarity` is above the threshold

### Files
- `swissarmyhammer-treesitter/src/unified.rs` (inline tests) or `tests/workspace_leader_reader.rs`

## Acceptance Criteria
- [ ] Test creates workspace with duplicate code across files and a non-duplicate control file
- [ ] Test runs full indexing with embedding enabled
- [ ] Test asserts at least one duplicate cluster is found
- [ ] Test asserts the cluster contains the expected duplicate files and excludes the unrelated file
- [ ] Test asserts cluster similarity is above threshold

## Tests
- [ ] `test_find_all_duplicates_detects_near_identical_functions` — the test IS the deliverable
- [ ] `cargo test -p swissarmyhammer-treesitter test_find_all_duplicates_detects` passes