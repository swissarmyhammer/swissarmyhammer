---
depends_on:
- 01KKPCVAZQMJ1Z0TV5HVCVW5DZ
position_column: done
position_ordinal: f880
title: 'Backend decides search strategy: use search_hybrid instead of search-only'
---
## What
The `search_entities` Tauri command currently calls `search_index.search()` which is fuzzy-only. It should call `search_hybrid()` so the backend decides whether to use fuzzy or semantic embedding based on query length and embedding availability.

The `search_hybrid` method already exists on EntitySearchIndex — short queries (≤3 words) use fuzzy first, long queries try semantic first. But it requires a `TextEmbedder` instance. For now, since no embedder is configured, the hybrid method will naturally fall back to fuzzy. But the call path should be in place so when embeddings are built, they're used automatically.

**Files:**
- `kanban-app/src/commands.rs` — change `search_index.search()` to `search_index.search_hybrid()` or keep fuzzy-only with a TODO for embedder wiring
- `kanban-app/src/state.rs` — optionally store an embedder on BoardHandle for future use

**Approach:**
Since `search_hybrid` is async and needs a `TextEmbedder`, and we don't have an embedder wired up yet, the simplest fix is: keep using `search()` (fuzzy) for now but document clearly that this is the entry point the backend uses, and it will switch to hybrid when an embedder is available. The architecture is already correct — backend owns the decision.

Alternatively, if the llama-embedding or ane-embedding crate is available, wire it in.

## Acceptance Criteria
- [ ] Backend search_entities is clearly documented as the search strategy decision point
- [ ] Path to hybrid search is clear (embedder wiring)

## Tests
- [ ] `cargo nextest run -p kanban-app`