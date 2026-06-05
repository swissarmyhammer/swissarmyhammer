---
assignees:
- claude-code
depends_on:
- 01KTC7Y50PEM427HQ79NW52WY4
position_column: todo
position_ordinal: '8e80'
project: semantic-search
title: 'Kanban task-embedding cache: SQLite sidecar in the board dir'
---
## What
Add a SQLite sidecar that caches per-task embedding vectors so the new `search tasks` op (next card) does not re-embed unchanged tasks on every call. kanban is file-based today (no SQLite), so this is NEW infra. It is self-healing: a model/dim change invalidates the cache, and an absent cache file (e.g. a fresh clone on another machine) is recreated and repopulated on demand.

New deps in `crates/swissarmyhammer-kanban/Cargo.toml`:
- `rusqlite = { workspace = true }` (bundled SQLite).
- `swissarmyhammer-search = { workspace = true }` — for `serialize_embedding`/`deserialize_embedding` blob helpers (created in the search-crate card). Do NOT hand-roll blob (de)serialization here.
- `swissarmyhammer-embedding = { workspace = true }` — the async lazy-loaded `qwen-embedding` default embedder (`Embedder` + `TextEmbedder` trait) that mean-pools long text. NOTE: this pulls llama/model/ane deps into kanban's tree for the first time; that is intended and accepted per the agreed design.

New file: `crates/swissarmyhammer-kanban/src/task/embedding_cache.rs` (registered as `mod embedding_cache;` in `crates/swissarmyhammer-kanban/src/task/mod.rs`).

Storage location: derive from the board dir. `KanbanContext::root()` returns the `.kanban` directory (see `crates/swissarmyhammer-kanban/src/context.rs`). Put the sidecar at `<root>/search-cache.sqlite3` (sibling of `board.yaml`/`tasks/`). Add a `KanbanContext` path helper `pub fn search_cache_path(&self) -> PathBuf { self.root.join(\"search-cache.sqlite3\") }` next to the other path helpers (`board_path`, `tasks_dir`, …).

Schema (created on open if absent):
- `embeddings(task_id TEXT NOT NULL, content_hash TEXT NOT NULL, vector BLOB NOT NULL, PRIMARY KEY(task_id, content_hash))`.
- `meta(key TEXT PRIMARY KEY, value TEXT)` storing the embedder model name (`Embedder::model_name`) and dimension. On open, if the stored model name or dim differs from the current embedder's, DROP/clear `embeddings` (self-healing invalidation) and rewrite the meta row.

API (sync rusqlite behind a small struct; embedding calls are async via the `Embedder`):
- `pub struct EmbeddingCache { conn: rusqlite::Connection }` with `open(path, model_name, dim) -> Result<Self>` (creates schema, runs model/dim invalidation). `open` MUST create the file and parent-safe schema when the path does not exist — this is the cold-start / fresh-clone rebuild path.
- `pub fn get(&self, task_id: &str, content_hash: &str) -> Option<Vec<f32>>` (cache hit -> deserialize blob).
- `pub fn put(&self, task_id: &str, content_hash: &str, vector: &[f32])` (serialize + upsert).
- `content_hash` is computed by the caller from the task's embedded content (title+description+tags) — define a `pub fn content_hash(text: &str) -> String` helper here using a stable hash (e.g. `xxhash-rust` xxh3, already a workspace dep, or sha2). Document the exact input contract so the search-op card composes the same string.

Lazy-fill belongs to the search-op card (it owns the `Embedder` and the corpus); THIS card delivers the store + invalidation + helpers, fully unit/integration tested without needing a real model (insert/get/put/hash and the model/dim-mismatch invalidation can all be tested with synthetic vectors and fake model-name/dim args).

### Gitignore (the cache is derived data, never committed)
The cache lives INSIDE `.kanban/`, which is only PARTIALLY tracked — the task `.md`/`.jsonl` files there are committed source, so we must NOT blanket-`*`-ignore `.kanban/` the way `.code-context/.gitignore` does. Instead, extend the existing `.kanban/.gitignore` writer in `crates/swissarmyhammer-kanban/src/board/init.rs` (it currently writes `mcp.log`) to also ignore the cache and its SQLite WAL sidecars:
- `search-cache.sqlite3`
- `search-cache.sqlite3-wal`
- `search-cache.sqlite3-shm`

(A single `search-cache.sqlite3*` glob is acceptable.) Make the writer idempotent — if `.kanban/.gitignore` already exists, ensure the lines are present without duplicating them (existing boards must gain the entries on next init/open, not only freshly-created ones). Do NOT rely on the stray `search.db` pattern in the root `.gitignore`; ignore explicitly in the board's own ignore file so the guarantee travels with the board dir.

## Acceptance Criteria
- [ ] `crates/swissarmyhammer-kanban/Cargo.toml` gains `rusqlite`, `swissarmyhammer-search`, `swissarmyhammer-embedding` deps; `cargo build -p swissarmyhammer-kanban` compiles.
- [ ] `EmbeddingCache::open` creates the sidecar at `<root>/search-cache.sqlite3` with the `embeddings` + `meta` tables.
- [ ] `put` then `get` for the same `(task_id, content_hash)` returns the exact vector (round-trips through the search-crate blob helpers).
- [ ] `get` for a missing `(task_id, content_hash)` returns `None`.
- [ ] Opening the cache with a different model name OR a different dim than the stored meta clears `embeddings` (self-healing) and updates `meta`.
- [ ] `KanbanContext::search_cache_path()` returns `<root>/search-cache.sqlite3`.
- [ ] **Cold start:** `EmbeddingCache::open` against a path that does NOT exist creates a fully usable cache (schema present, `put`/`get` work) — no pre-existing file required. This is the cross-machine / fresh-clone rebuild guarantee at the store layer.
- [ ] **Gitignore:** after board init (and on open of a pre-existing board), `.kanban/.gitignore` contains `search-cache.sqlite3` (+ `-wal`/`-shm`, or the glob), the entries are not duplicated on repeat, and `git check-ignore` would match the cache file. `.kanban/`'s tracked task files remain un-ignored.

## Tests
- [ ] Unit/integration tests in `embedding_cache.rs` `#[cfg(test)] mod tests` using a `TempDir`: put/get round-trip; get-miss returns None; content_hash is stable for equal input and differs for different input; model-name-mismatch invalidation clears rows; dim-mismatch invalidation clears rows; reopening with the SAME model/dim preserves rows.
- [ ] Cold-start test: `open` on a non-existent path inside a fresh `TempDir` succeeds and supports `put`/`get` (proves the rebuild-on-checkout path at the store layer).
- [ ] Gitignore test (in `board/init.rs` tests or `embedding_cache.rs`): after writing/refreshing `.kanban/.gitignore`, assert it contains the `search-cache.sqlite3` (+ sidecar) patterns, that running the writer twice does not duplicate lines, and that the committed task files are still not ignored.
- [ ] `cargo test -p swissarmyhammer-kanban embedding_cache` and the gitignore test pass (all new tests green, no real model required).

## Workflow
- Use `/tdd` — write failing tests first, then implement to pass.