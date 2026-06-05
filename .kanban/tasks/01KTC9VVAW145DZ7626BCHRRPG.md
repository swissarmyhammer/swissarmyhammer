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
- `impl Execute<KanbanContext, KanbanError> for SearchTasks` — `async fn execute` (the `#[operation]` path already uses `async_trait`).

Corpus build (mirror `ListTasks::execute` for the scoping half — reuse `enrich_all_task_entities`, `EntitySlugRegistry`, `TaskFilterAdapter`, `parse_filter_expr` from `crate::task::shared` / `crate::task_helpers`; exclude the terminal/done column the same way `list tasks` does unless a column is implied by the filter): produce the in-scope set of enriched task entities. Keep this scoping + Doc-build + map-back logic in small helper fns that are testable WITHOUT embedding.
- For each in-scope task build a `swissarmyhammer_search::Doc`: `id` = task id; `fields` = `[ Field { weight: <high>, text: title }, Field { weight: <low>, text: description }, Field { weight: <mid>, text: tags joined } ]`; `embedding` = the task's cached vector. NOTE: tags are a lexical Doc field (BM25/trigram) but are NOT part of the embedded text (see below).

Embeddings (real model, always — lazy-fill via the cache card's store):
- The embedder is ALWAYS used; there is NO lexical-only fallback. Obtain a PROCESS-LIFETIME embedder loaded at most once (a `OnceCell`/shared handle), NOT `Embedder::default().await.load().await` per call — reloading the qwen-embedding model on every search is a multi-second cliff that makes interactive search unusable. If the embedder cannot load, return a `KanbanError` (do not silently degrade). Log via `tracing`, never `eprintln!`.
- Open `EmbeddingCache` at `ctx.search_cache_path()` keyed by the embedder's `model_name()` + dim.
- For each in-scope task compute the embed text via the cache card's single canonical builder `embedding_cache::task_embedding_text(title, description)` (tags excluded), and `content_hash` of that SAME string. On cache MISS, embed via `embedder.embed_text(text).await` and `put` it; on HIT, use the cached vector. Lazy-fill: only misses pay the embed cost; the store self-heals on model/dim change.
- Embed the `query` string once for the query embedding.

Rank + map back:
- Build `Query { text: query, embedding: Some(query_vec), weights: SignalWeights::default(), top_k: top_k.unwrap_or(10), min_score: None }` and call `swissarmyhammer_search::search(&docs, &query)`. (Default top_k 10 to match `list tasks`' `DEFAULT_PAGE_SIZE` — keep AI tool results lean.)
- Map each `Hit` back to the enriched task JSON (via `task_entity_to_rich_json`) and attach the hit's `score` + `signals`. Return `{ "tasks": [ {<enriched task>, "score": .., "signals": {..}} , ... ], "count": N }` (shape consistent with `list tasks` plus score/signals).

Right-sizing: if op + cache wiring + corpus build exceeds ~5 subtasks or 500 LOC, split the lazy-fill embedding loop into a helper module under `task/` and link with depends_on. Keep this card focused on: op struct + corpus scoping + Doc build + embed-or-cache loop + map-back.

## Acceptance Criteria
- [ ] `SearchTasks` op exists with `query` (required), optional `filter`, optional `top_k`, and is exported from `task/mod.rs`.
- [ ] An optional DSL `filter` scopes the corpus using the same filter path as `list tasks` (e.g. `#bug` restricts which tasks are ranked); without a filter the whole non-done board is the corpus.
- [ ] Each in-scope task becomes a `Doc` with title (high weight), description (low), tags (mid) as lexical fields, and its cached embedding; the embedded text is `task_embedding_text(title, description)` (tags excluded).
- [ ] The embedder is loaded at most once per process (not per call); cache miss embeds and stores; cache hit reuses; a second `search tasks` call does not re-embed unchanged tasks.
- [ ] If the embedder cannot load, the op returns a `KanbanError` — it does NOT fall back to a lexical-only mode.
- [ ] Response is ranked `Hit`s mapped to enriched task JSON carrying `score` + `signals`.

## Tests
- [ ] Unit tests in `search.rs` `#[cfg(test)] mod tests` using a `TempDir` board (pattern from `list.rs` tests: `InitBoard` + `AddTask`) for the NON-embedding logic — no model required:
  - filter-scoping: `#bug` (or similar) narrows the in-scope set before ranking; no filter = whole non-done board.
  - Doc construction: a task maps to a `Doc` with the right fields/weights and `task_embedding_text` excludes tags.
  - map-back: a `Vec<Hit>` maps to the `{tasks:[{..,score,signals}], count}` response shape.
- [ ] `cargo test -p swissarmyhammer-kanban search` passes (these tests need no real model; the full real-embedder embed+rank is proven in the kanban search e2e card).

## Workflow
- Use `/tdd` — write failing tests first, then implement to pass.