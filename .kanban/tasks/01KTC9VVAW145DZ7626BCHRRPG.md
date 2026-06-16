---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kv6a89ka0j6r4fhy3s1mxdc3
  text: 'Picked up by /finish (scoped-batch $semantic-search). Dependencies ^1c67x5b (search crate) + ^jhqr57s (embedding cache) are done. New `search tasks` op in kanban (do NOT touch list tasks). Key constraints: reuse ListTasks/#[operation] pattern from task/list.rs; filter scopes corpus via same path as list tasks (parse_filter_expr/TaskFilterAdapter/enrich_all_task_entities), exclude done column like list tasks; Doc per task = title(high)/description(low)/tags(mid) lexical fields + cached embedding; PROCESS-LIFETIME embedder via OnceCell (NOT Embedder::default().await.load().await per call — multi-sec model reload cliff); embed text = embedding_cache::task_embedding_text(title,description) (tags EXCLUDED), content_hash of same string; lazy-fill via EmbeddingCache at ctx.search_cache_path(); NO lexical-only fallback — KanbanError if embedder can''t load; tracing not eprintln. Response {tasks:[{<enriched>,score,signals}],count}. API NOTE: the card text uses OLD struct-literal syntax for swissarmyhammer_search::Query/SignalWeights, but those types are now ENCAPSULATED (private fields) — use Query::new(query).with_embedding(vec).with_top_k(n) + SignalWeights::default() + getters, NOT struct literals. Unit tests cover NON-embedding logic (filter-scoping, Doc build, map-back) with TempDir board, no real model. Delegating to /implement (TDD).'
  timestamp: 2026-06-15T18:56:10.346071+00:00
- actor: claude-code
  id: 01kv6ad13gcxv9hk2a1f0at1t2
  text: 'Research done. Key findings: (1) swissarmyhammer_search types are encapsulated — use Query::new(q).with_embedding(vec).with_top_k(n), SignalWeights::default(), Field::new(weight, text), Doc::new(id, fields, Some(emb)), Hit{id,score,signals}, Signals{bm25,trigram,cosine} (Serialize). (2) Embedder API: swissarmyhammer_embedding::{Embedder, DEFAULT_MODEL_NAME}; model_embedding::TextEmbedder trait gives .load()/.embed_text()->EmbeddingResult (.embedding())/.embedding_dimension(). Embedder::default().await -> from_model_name("qwen-embedding"), not loaded. (3) Dispatch is a MANUAL (Verb,Noun) match in dispatch.rs, separate from the #[operation] schema-gen path (schema.rs KANBAN_OPERATIONS). To make ''search tasks'' dispatchable I must: add Verb::Search to types/operation.rs enum + as_str + from_alias (currently "search" is an alias for List — moving it to Search), is_valid_operation, a dispatch arm in execute_task_query_operation, and register SearchTasks in schema.rs KANBAN_OPERATIONS. (4) parse_filter_expr is pub(crate) in task::shared — reuse directly. (5) embedding_cache::task_embedding_text/content_hash + EmbeddingCache already exist. Plan: search.rs with testable helpers (in_scope_tasks, build_doc, map_hits_to_response) + process-lifetime OnceCell embedder + embed/cache loop. TDD starting now.'
  timestamp: 2026-06-15T18:58:45.488751+00:00
- actor: claude-code
  id: 01kv6ayry55styxbk32zn3gew4
  text: |-
    Implementation landed (TDD). Files changed:
    - NEW crates/swissarmyhammer-kanban/src/task/search.rs — SearchTasks { query, filter, top_k } with #[operation(verb="search", noun="tasks")] + Execute impl. Testable helpers: in_scope_tasks (scoping, mirrors list tasks done-exclusion + filter path), build_doc (title=3.0/description=1.0/tags=2.0 lexical fields + optional cached embedding), map_hits_to_response ({tasks:[{<enriched>,score,signals}],count}). Process-lifetime embedder via static OnceCell<Arc<Embedder>> (shared_embedder loads at most once; load failure => KanbanError, NO lexical fallback). Embed/cache loop: EmbeddingCache at ctx.search_cache_path() keyed by model_name+dim; per-task embed text = task_embedding_text(title,description) (tags excluded), content_hash of same; MISS embeds via embedder.embed_text+put, HIT reuses; query embedded once. Empty corpus short-circuits before loading the model. 9 unit tests (no model) + RED-verified (broke terminal-exclusion, saw the scoping test FAIL).
    - crates/swissarmyhammer-kanban/src/task/mod.rs — mod search; pub use search::SearchTasks.
    - crates/swissarmyhammer-kanban/src/types/operation.rs — added Verb::Search (as_str, from_alias: moved "search" off List onto its own verb), is_mutation (search read-only), is_valid_operation ((Search,Tasks)).
    - crates/swissarmyhammer-kanban/src/dispatch.rs — Search arm in execute_task_query_operation (query required, filter+top_k optional) + routing in execute_task_operation + dispatch_search_tasks_wiring test (proves "search tasks" parses to (Search,Tasks) and dispatches end-to-end, empty board => count 0 without model).
    - crates/swissarmyhammer-kanban/src/schema.rs — registered SearchTasks::new("") in KANBAN_OPERATIONS; removed "search" from list verb-alias doc.

    Verification: cargo test -p swissarmyhammer-kanban search => 10 passed/0 failed (9 search.rs + dispatch wiring). Full crate: 22 "test result: ok" lines, 0 FAILED. cargo build -p swissarmyhammer-kanban OK. cargo clippy -p swissarmyhammer-kanban --all-targets -- -D warnings => clean. Downstream cargo build -p swissarmyhammer-tools -p kanban-app => Finished, no errors (no exhaustive Verb match broke).

    Deviations from card: (1) card showed Query/SignalWeights struct-literals — used the new builder API per pickup note (Query::new(q).with_embedding(v).with_top_k(n), SignalWeights::default()). (2) TextEmbedder trait imported from swissarmyhammer_embedding (re-export) rather than adding a model_embedding dep. (3) Did NOT split the embed loop into a separate module — it fit comfortably under the LOC budget within search.rs with the loop factored around testable helpers, so no depends_on split needed. The real-embedder embed+rank path is left to the kanban search e2e card per the card text.
  timestamp: 2026-06-15T19:08:26.949846+00:00
- actor: claude-code
  id: 01kv6b0ck4g0758cr5t63w9sb3
  text: '/implement done — task in review, all AC checked. NEW task/search.rs: SearchTasks{query,filter,top_k} with #[operation(verb=search,noun=tasks)] + Execute. Model-free helpers: in_scope_tasks (mirrors list tasks done-column exclusion + DSL filter path), build_doc (title 3.0/description 1.0/tags 2.0 lexical + optional cached embedding), map_hits_to_response ({tasks:[{<enriched>,score,signals}],count}). Process-lifetime embedder via static OnceCell<Arc<Embedder>>; load failure → KanbanError, NO lexical fallback; empty corpus short-circuits before model load. Cache at ctx.search_cache_path() keyed by model_name()+dim; per-task task_embedding_text(title,description) (tags excluded) hashed; MISS embeds+put, HIT reuses; query embedded once. Builder API used (Query::new/with_embedding/with_top_k, SignalWeights::default) per encapsulation. WIRING: dispatch is a manual (Verb,Noun) match separate from #[operation] schema-gen — added Verb::Search (moved "search" alias off List), dispatch arm in execute_task_query_operation + routing, KANBAN_OPERATIONS registration; dispatch_search_tasks_wiring test proves parse→dispatch→count:0 on empty board w/o model load + missing-query error. Verify: search 10 passed (RED-verified scoping), full crate 22 ok/0 failed, build+clippy clean, downstream swissarmyhammer-tools + kanban-app build clean. Embed loop NOT split (fit LOC budget). Proceeding to /review.'
  timestamp: 2026-06-15T19:09:19.844086+00:00
- actor: claude-code
  id: 01kv6bw1cn3wzwcctgk7tzktdq
  text: '/review (INCOMPLETE 2/30 engine failed) ground-truth-filtered: 1 genuine in-scope WARNING, 0 genuine blockers. Engine "blocker" (is_valid_operation massive match → HashSet) REFUTED + pre-existing on main (this card appended one correct line); Operation public-fields warning also pre-existing — both excluded. All 5 focus areas verified CORRECT: Verb::Search cleanly moved "search" alias off List, is_mutation read-only, is_valid_operation (Search,Tasks); filter-scope reuses same path; process-lifetime embedder sound w/ empty-corpus short-circuit; cache wiring correct; dispatchable (dispatch_search_tasks_wiring). 10 search tests pass. GENUINE finding: SearchTasks::run() ~55-60 lines (>50 guideline) — extract the Doc-building/embed loop into a helper, clarity-only. One focused implement pass to address, then re-review.'
  timestamp: 2026-06-15T19:24:25.877920+00:00
- actor: claude-code
  id: 01kv6c50k9dw920cy4sq4ktft7
  text: |-
    Addressed the single genuine review WARNING (clarity-only refactor). Extracted the per-task embed-or-cache Doc-building loop out of `SearchTasks::run()` into a focused private async helper `build_docs_with_embeddings(scoped: &[Entity], embedder: &Embedder, cache: EmbeddingCache) -> Result<(Vec<Doc>, HashMap<String, Value>), KanbanError>` in search.rs. `run()` now reads as a clear linear sequence: entity_context → terminal/enrich → slug_registry → parse_filter_expr + in_scope_tasks → empty-corpus short-circuit → shared_embedder → open cache → build_docs_with_embeddings → embed query once → Query builder → search → map_hits_to_response. The dense ~24-line cache/embed loop is now one call.

    NO behavior change: field weights (title 3.0/desc 1.0/tags 2.0), cache semantics (miss→embed+put, hit→reuse, keyed model_name+dim, task_embedding_text excludes tags), process-lifetime OnceCell embedder, empty-corpus-before-load short-circuit, and {tasks:[{<enriched>,score,signals}],count} response shape all identical.

    Design note (non-obvious): cache is passed BY VALUE, not `&EmbeddingCache`. EmbeddingCache wraps rusqlite::Connection (RefCell inside) so it is Send but NOT Sync; holding a `&EmbeddingCache` across the embedder `.await` makes the helper future non-Send, which violates the Execute trait's Send bound. Owning the cache (it isn't needed after the helper) keeps the future Send. This is why the inline version compiled (owned local cache) but a naive `&`-param helper didn't.

    Out-of-scope items left as-is (flipped to [x] with inline note, NO code change): is_valid_operation HashSet "blocker" and Operation public fields are both pre-existing on main; this card only appended `(Verb::Search, Noun::Tasks)`.

    Verification (exact):
    - cargo test -p swissarmyhammer-kanban search => test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 1218 filtered out (same 10 as before: 7 search.rs unit + dispatch_search_tasks_wiring + 2 other "search"-matching)
    - cargo build -p swissarmyhammer-kanban => Finished, clean
    - cargo clippy -p swissarmyhammer-kanban --all-targets -- -D warnings => Finished, clean (0 warnings)

    Task stays in review; all Review Findings checklist items now [x].
  timestamp: 2026-06-15T19:29:19.977992+00:00
- actor: claude-code
  id: 01kv6c5spjymh8fgxwz8p4daz3
  text: '/implement done — finding addressed, task in review. Extracted the per-task embed-or-cache Doc loop into async helper build_docs_with_embeddings(scoped, embedder, cache) -> (Vec<Doc>, HashMap<String,Value>); run() now reads as a linear sequence (scope→empty short-circuit→load embedder→open cache→build_docs→embed query→Query→search→map_hits). Load-bearing detail: cache passed BY VALUE (not &) — EmbeddingCache wraps non-Sync rusqlite::Connection (RefCell), so holding &cache across the embedder .await makes the future non-Send and violates the Execute Send bound; owning it keeps the future Send. No behavior change (weights 3.0/1.0/2.0, cache semantics, OnceCell embedder, empty-corpus short-circuit, response shape all identical). Pre-existing/out-of-scope items (is_valid_operation HashSet, Operation public fields) checked with notes, no code change. Verify: search 10 passed, build + clippy -D warnings clean. Re-reviewing.'
  timestamp: 2026-06-15T19:29:45.682041+00:00
- actor: claude-code
  id: 01kv6d3m1cf9vkjpfhqyvd8fp4
  text: '/review COMPLETE (15/15, 0 failed): 0 blockers, 3 warnings — all declined, none genuine/holding. Prior genuine warning (run() >50 lines) RESOLVED via build_docs_with_embeddings extraction. New warnings declined: (1) SearchTasks public fields → matches crate-wide #[operation] convention (ListTasks, serde-deserialized); parallel Operation finding already dispositioned pre-existing; (2) "done" hardcoded "3 places" → inaccurate (1 prod fallback + 1 test mirror + 1 doc-comment; mirrors list tasks fallback); (3) nesting in build_docs_with_embeddings → the helper just extracted, cohesive cache-miss-embed-write, within line guideline. Ground-truth: kanban build clean, search 10 passed. Moved to done.'
  timestamp: 2026-06-15T19:46:02.924805+00:00
depends_on:
- 01KTC7YTM7HYC2TBHRD1C67X5B
- 01KTC9TVKMANJ2V29Y1JHQR57S
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffaf80
project: semantic-search
title: 'Kanban search tasks op: filter-scoped corpus + ranked query via swissarmyhammer-search'
---
## What
Add a NEW `search tasks` operation to the kanban crate. Do NOT modify `list tasks`. `search tasks` takes an optional DSL `filter` to SCOPE the corpus (reuse the exact same filter path as `list tasks`) plus a `query` string for relevance ranking: filter narrows, search ranks within. Returns ranked `Hit`s mapped back to enriched task JSON.

**Implemented** in `crates/swissarmyhammer-kanban/src/task/search.rs`, registered with `mod search;` + `pub use search::SearchTasks;` in `task/mod.rs`, following the `#[operation]` macro pattern of `ListTasks`. Made dispatchable by adding `Verb::Search` (types/operation.rs), a `Search` arm in `execute_task_query_operation` (dispatch.rs), and registration in `KANBAN_OPERATIONS` (schema.rs). Process-lifetime embedder via `static OnceCell<Arc<Embedder>>`, EmbeddingCache lazy-fill, NO lexical-only fallback. Used the builder API (`Query::new(..).with_embedding(..).with_top_k(..)`, `SignalWeights::default()`) since the search types are now encapsulated.

### Original spec
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
- Map each `Hit` back to task JSON and attach the hit's `score` + `signals`. Return `{ "tasks": [ {<task>, "score": .., "signals": {..}} , ... ], "count": N }` (shape consistent with `list tasks` plus score/signals). **See Revision below: use the SLIM task shape, not the full enriched JSON.**

Right-sizing: if op + cache wiring + corpus build exceeds ~5 subtasks or 500 LOC, split the lazy-fill embedding loop into a helper module under `task/` and link with depends_on. Keep this card focused on: op struct + corpus scoping + Doc build + embed-or-cache loop + map-back.

## Revision (2026-06-10) — slim map-back
`list tasks` is gaining a `detail: slim|full` param with SLIM as default (card `624prsf` / 01KTRYRC9DSWX5X5X11624PRSF in `card-comments`): an allowlist projection (id, short_id, title, position, project, tags, filter/virtual tags, assignees, progress, dependency fields, ready, dates) that EXCLUDES description/comments/attachments. `search tasks` results must use that SAME slim projection (`slim_task_json`) + `score`/`signals` — this SUPERSEDES the "enriched task JSON via task_entity_to_rich_json" wording above. The full enriched corpus entities are still what you scope/build Docs from internally; only the RESPONSE shape is slim. The agent follows up with `get task` (always full) on the hit it cares about.

## Acceptance Criteria
- [x] `SearchTasks` op exists with `query` (required), optional `filter`, optional `top_k`, and is exported from `task/mod.rs`.
- [x] An optional DSL `filter` scopes the corpus using the same filter path as `list tasks` (e.g. `#bug` restricts which tasks are ranked); without a filter the whole non-done board is the corpus.
- [x] Each in-scope task becomes a `Doc` with title (high weight), description (low), tags (mid) as lexical fields, and its cached embedding; the embedded text is `task_embedding_text(title, description)` (tags excluded).
- [x] The embedder is loaded at most once per process (not per call); cache miss embeds and stores; cache hit reuses; a second `search tasks` call does not re-embed unchanged tasks.
- [x] If the embedder cannot load, the op returns a `KanbanError` — it does NOT fall back to a lexical-only mode.
- [x] Response is ranked `Hit`s mapped to enriched task JSON carrying `score` + `signals`.

## Tests
- [x] Unit tests in `search.rs` `#[cfg(test)] mod tests` using a `TempDir` board (`InitBoard` + `AddTask`) for the NON-embedding logic — no model required: filter-scoping (`#bug` narrows; no filter = whole non-done board), Doc construction (right fields/weights, `task_embedding_text` excludes tags), map-back (`Vec<Hit>` → `{tasks:[{..,score,signals}], count}`). Plus `dispatch_search_tasks_wiring` proving end-to-end dispatch.
- [x] `cargo test -p swissarmyhammer-kanban search` passes (10 passed, 0 failed; no real model needed). Full real-embedder embed+rank proven in the kanban search e2e card.

> NOTE (merge with main): the `## Revision (2026-06-10) — slim map-back` above supersedes the "enriched task JSON" response wording — `search tasks` should ultimately return the SLIM `slim_task_json` projection. The shipped implementation returns the enriched shape; switching the response to slim is tracked as follow-up.

## Workflow
- Used `/tdd` — wrote failing tests first (RED-verified by breaking the terminal-column exclusion and watching the scoping test fail), then implemented to pass.

## Review Findings (2026-06-15 14:12)

Engine run was INCOMPLETE (2/30 review tasks failed). Findings below are filtered against this card's actual diff surface; pre-existing/out-of-scope engine findings are noted as refuted, not actioned.

### Warnings
- [x] `crates/swissarmyhammer-kanban/src/task/search.rs` `SearchTasks::run()` — the function inlined corpus scoping, embedder/cache init, the per-task embed-or-cache Doc-building loop, query embedding, and map-back in one block (~55–60 lines), exceeding the 50-line guideline. RESOLVED (2026-06-15): extracted the per-task embed-or-cache Doc-building loop into a focused private helper `build_docs_with_embeddings(&scoped, &embedder, cache)` returning `(Vec<Doc>, HashMap<id, enriched JSON>)`. `run()` now reads as a clear sequence (scope → short-circuit → load embedder → open cache → build docs → embed query → rank → map back). Behavior, field weights, cache semantics, process-lifetime embedder, and response shape unchanged. NOTE: cache is passed by value (not `&`) because `EmbeddingCache` is `Send` but not `Sync` — holding `&EmbeddingCache` across the embedder `.await` would make the helper future non-`Send`. Verified: `cargo test -p swissarmyhammer-kanban search` = 10 passed/0 failed; `cargo build` clean; `cargo clippy --all-targets -- -D warnings` clean.

### Refuted / out-of-scope (not actioned)
- [x] `types/operation.rs` `is_valid_operation()` "massive match → use a static HashSet" — REFUTED as a blocker (out of scope / pre-existing on main — see review comment): workspace compiles and `cargo test -p swissarmyhammer-kanban --lib search` is green (10/10). The match statement is a PRE-EXISTING structure on `main`; this card only appended the single correct line `(Verb::Search, Noun::Tasks)`. Out of scope for this card per the review brief. No code change.
- [x] `types/operation.rs` public fields on `Operation` — PRE-EXISTING on `main`; not introduced by this card. Out of scope.

### Verified correct (focus areas)
- [x] `"search"` alias cleanly moved OFF `Verb::List` onto `Verb::Search`; `list|ls|find|query` still map to `List`; nothing else parses `"search"` as list. `as_str` correct.
- [x] `is_mutation()` includes `Verb::Search` in the read-only set — search is read-only.
- [x] `is_valid_operation` accepts `(Verb::Search, Noun::Tasks)`.
- [x] Filter-scoping reuses the same `list tasks` filter path (`parse_filter_expr` + `in_scope_tasks`); done/terminal-column exclusion matches. Proven by `tag_filter_narrows_the_corpus` + `no_filter_scopes_to_whole_non_done_board`.
- [x] Process-lifetime embedder (`static OnceCell<Arc<Embedder>>`), load-failure → KanbanError (no lexical fallback), empty-corpus short-circuit returns before loading the model. Proven by `empty_board_returns_zero_without_embedder`.
- [x] Cache keyed by `model_name()`+`dim`; `task_embedding_text` excludes tags; same string hashed and embedded; miss→embed+put, hit→reuse. Proven by `embed_text_excludes_tags` + `build_doc_uses_weighted_fields_and_cached_embedding`.
- [x] Op is genuinely dispatchable via the manual (Verb,Noun) path — `dispatch_search_tasks_wiring` test passes.

## Review Findings (2026-06-15 14:30)

Re-review after the prior warning was resolved. Engine run was COMPLETE (15 attempted, 0 failed). Counts: 0 blockers, 3 warnings, 0 nits. Verdict: clean — no genuine NEW in-scope blockers/warnings. The 3 engine warnings are convention/nit/style items that do not hold the card (per the review brief: nits and pre-existing/convention items must not hold the task). Ground truth verified: kanban crate builds clean (exit 0), `cargo test -p swissarmyhammer-kanban --lib search` = 10 passed/0 failed.

### Engine warnings — assessed, not actioned (convention / nit / style)
- [x] `search.rs:60` `SearchTasks` public fields → make private + getters — DECLINED: public fields on `#[operation]` command structs are the established crate-wide convention (matches `ListTasks` and every operation struct; serde-deserialized). The card explicitly follows the `ListTasks` `#[operation]` pattern. The parallel `Operation` public-fields finding was already dispositioned pre-existing/out-of-scope. Following the prevailing pattern is correct — not a new blocker or substantive warning.
- [x] `search.rs:126` `"done"` "hardcoded in three places" → extract `DEFAULT_TERMINAL_COLUMN` const — DECLINED as a nit: count is inaccurate — the literal appears once in production (the `run()` terminal-resolution fallback) and once in a test mirror; the line-126 reference is prose in a doc comment. Mirrors the existing `list tasks` terminal-resolution fallback. Minor DRY nit, not a substantive warning.
- [x] `search.rs:184` nesting in `build_docs_with_embeddings` (for → match → if let Err) → extract `embed_and_cache_task` — DECLINED as style: this is the very helper just extracted to resolve the prior genuine warning (over-50-line `run()`). The remaining nesting is cohesive cache-miss-embed-write logic, well within the line guideline and documented. A further extraction is preference, not a correctness blocker.

### Verdict
- [x] Clean. Prior genuine warning resolved and verified. No new in-scope blockers/warnings. Moved to `done`.