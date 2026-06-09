---
assignees:
- claude-code
position_column: review
position_ordinal: '8380'
title: 'Review memory: route code-context ranked retrieval through the search crate (bounded top-k ~128)'
---
## Problem
code-context re-implements ranked embedding retrieval that the search crate (`swissarmyhammer-entity-search`) already owns:
- `search_code.rs::rank_loaded` (over `LoadedChunk`) ≈ `entity-search::semantic::semantic_search`.
- `find_duplicates.rs::find_duplicates_in` is worse: it clones EVERY chunk above `min_similarity` into a `ChunkRef` (full `text`) before truncating to `max_per_chunk = 5`. On a large/boilerplate repo a hot source chunk matches a huge fraction of the corpus → big transient allocation, multiplied per source chunk per changed file.

This is duplicate-but-different code (forbidden) AND an unbounded allocation.

## Fix
Make `swissarmyhammer-code-context` depend on `swissarmyhammer-entity-search` and route duplicate/similar ranking through it. Retrieval must be bounded to a top-k candidate pool of ~128 (the rank-fusion candidate set) via a size-k min-heap, NOT a whole-corpus scan/clone. Each probe returns its smaller final set (similar: 5, duplicates: max_per_chunk) from those candidates.

The reusable ranked-retrieval primitive lives in the search crate; code-context consumes it. No third copy.

#review #memory #cleanup