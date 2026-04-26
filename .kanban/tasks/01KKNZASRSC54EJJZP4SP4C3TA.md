---
assignees: []
position_column: done
position_ordinal: '9280'
title: 'Behavioral change: cosine_similarity return range shifted for some consumers'
---
swissarmyhammer-code-context/src/ops/search_code.rs:72, swissarmyhammer-tools/src/mcp/tools/shell/state.rs:22\n\nThe old pure-Rust implementations returned cosine similarity in range [0.0, 1.0] (dot/norms). The new simsimd-based canonical version returns `1.0 - distance` which is in range [-1.0, 1.0] for non-normalized vectors. For normalized embedding vectors (which these consumers use), the ranges overlap and the values are identical. But if any consumer ever receives non-normalized vectors, results could differ from what the old code produced (e.g., opposite vectors: old returned 0.0 via `dot/denom`, new returns negative via `1.0 - 2.0 = -1.0`).\n\nThe existing tests in search_code.rs cover `test_cosine_similarity_opposite` which tests `a = [1,0]` vs `b = [-1,0]` — verify this test still passes with the SIMD version (it does, since the old test expected `-1.0` which matches the SIMD behavior). Low risk since all embeddings in practice are normalized, but worth noting.