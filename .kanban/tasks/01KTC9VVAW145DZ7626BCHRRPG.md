---
assignees:
- claude-code
depends_on:
- 01KTC7YTM7HYC2TBHRD1C67X5B
- 01KTC9TVKMANJ2V29Y1JHQR57S
position_column: todo
position_ordinal: 8f80
project: semantic-search
title: 'Kanban search tasks op: filter-scoped corpus + ranked query via swissarmyhammer-search'
---
## What
Add a NEW `search tasks` operation to the kanban crate. Do NOT modify `list tasks`. `search tasks` takes an optional DSL `filter` to SCOPE the corpus (reuse the exact same filter path as `list tasks`) plus a `query` string for relevance ranking: filter narrows, search ranks within. Returns ranked `Hit`s mapped back to enriched task JSON.

New file: `crates/swissarmyhammer-kanban/src/task/search.rs`, registered with `mod search;` + `pub use search::SearchTasks;` in `crates/swissarmyhammer-kanban/src/task/mod.rs`. Follow the `#[operation]` macro pattern of `crates/swissarmyhammer-kanban/src/task/list.rs` (`ListTasks`):
- `#[operation(verb = "search", noun = "tasks", description = "Search tasks by relevance, optionally scoped by a DSL filter")]` on `pub struct SearchTasks { pub query: String, pub filter: Option<String>, pub top_k: Option<usize> }`.
- `impl Execute<KanbanContext, KanbanError> for SearchTasks`.

Corpus build (mirror `ListTasks::execute` for the scoping half — reuse `enrich_all_task_entities`, `EntitySlugRegistry`, `TaskFilterAdapter`, `parse_filter_expr` from `crate::task::shared` / `crate::task_helpers`; exclude the terminal/done column the same way unless a column is implied by the filter): produce the in-scope set of enriched task entities.
- For each in-scope task build a `swissarmyhammer_search::Doc`: `id` = task id; `fields` = `[ Field { weight: <high>, text: title }, Field { weight: <low>, text: description }, Field { weight: <mid>, text: tags joined } ]`; `embedding` = the task's cached vector.

Embeddings (lazy-fill, self-healing — uses the cache card's store):
- Construct the `Embedder` via `swissarmyhammer_embedding::Embedder::default().await` (the `qwen-embedding` default), `load().await` once. Open `EmbeddingCache` at `ctx.search_cache_path()` keyed by the embedder's `model_name()` + dim.
- For each in-scope task compute `content_hash(content)` where `content` is the SAME title+description+tags string contract the cache card documents. On cache MISS, embed now via `embedder.embed_text(content).await`, `put` into the cache. On HIT, use the cached vector. This is lazy-fill: only cache-misses pay the embed cost; the store self-heals on model/dim change.
- Embed the `query` string once for the query embedding.
- If the embedder cannot load (no model / offline), degrade gracefully: build the `Query` with `embedding: None` and `Doc`s with `embedding: None` so `search()` fuses bm25+trigram only (the search crate already handles absent signals). Log via `tracing`, never `eprintln!`.

Rank + map back:
- Build `Query { text: query, embedding: <query vec or None>, weights: SignalWeights::default(), top_k: top_k.unwrap_or(<sensible default, e.g. 20>), min_score: None }` and call `swissarmyhammer_search::search(&docs, &query)`.
- Map each `Hit` back to the enriched task JSON (via `task_entity_to_rich_json`) and attach the hit's `score` + `signals`. Return `{ "tasks": [ {<enriched task>, "score": .., "signals": {..}} , ... ], "count": N }` (shape consistent with `list tasks` plus score/signals).

Right-sizing: if the op + cache wiring + corpus build exceeds ~5 subtasks or 500 LOC, split the lazy-fill embedding loop into a helper module under `task/` and link with depends_on. Aim to keep this card focused on: op struct + corpus scoping + Doc build + embed-or-cache loop + map-back.

## Acceptance Criteria
- [ ] `SearchTasks` op exists with `query` (required), optional `filter`, optional `top_k`, and is exported from `task/mod.rs`.
- [ ] An optional DSL `filter` scopes the corpus using the same filter path as `list tasks` (e.g. `#bug` restricts which tasks are ranked); without a filter the whole non-done board is the corpus.
- [ ] Each in-scope task becomes a `Doc` with title (high weight), description (low), tags (mid), and its cached embedding.
- [ ] Cache miss embeds and stores; cache hit reuses; a second `search tasks` call does not re-embed unchanged tasks.
- [ ] When the embedder is unavailable, the op still returns ranked results using bm25+trigram only (cosine signal absent), and logs via tracing.
- [ ] Response is ranked `Hit`s mapped to enriched task JSON carrying `score` + `signals`.

## Tests
- [ ] Unit/integration tests in `search.rs` `#[cfg(test)] mod tests` using a `TempDir` board (pattern from `list.rs` tests: `InitBoard` + `AddTask`): filter-scoping narrows the corpus before ranking; ranking orders an exact-title match above an unrelated task using bm25/trigram (with `embedding: None` so the test needs NO real model); a second call hits the cache (assert via a spy/seeded cache or by asserting the sidecar file has the row); response carries `score`/`signals`.
- [ ] `cargo test -p swissarmyhammer-kanban search` passes (no real model required for the ranking/scoping tests; gate any real-embedder test behind the same convention the embedding crate uses).

## Workflow
- Use `/tdd` — write failing tests first, then implement to pass.