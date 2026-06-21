---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kv64f3zg8kv359njazk2f93m
  text: 'Picked up by /finish (scoped-batch $semantic-search) after sibling card ^1c67x5b landed in done. Task is in `review`, all acceptance criteria checked, progress 1.0. NOTE: this card''s files (board/init.rs, context.rs, task/embedding_cache.rs) were heavily scrutinized and fixed during ^1c67x5b''s whole-tree review loop — the init.rs merge-driver/gitignore code (unregister_merge_drivers last-line bug, write_driver_file helper, remove_config_section helper, named permission consts, REQUIRED const) and context.rs are already clean. Plan: verify `cargo test -p swissarmyhammer-kanban` green (incl embedding_cache + gitignore tests), then /review to drive review → done.'
  timestamp: 2026-06-15T17:15:02.512416+00:00
- actor: claude-code
  id: 01kv64jzrmxvr4yc9sjmrb3b4m
  text: '/test green: `cargo test -p swissarmyhammer-kanban` → 1450 passed, 0 failed. embedding_cache.rs 8 tests pass (round-trip, get-miss, prune-prior-rows, tags-excluded/hash-stable, model-name mismatch clears, dim mismatch clears, reopen preserves, cold-start on nonexistent path); board::init gitignore test ensure_gitignore_entries_is_idempotent passes; context.rs search_cache_path test_paths passes. clippy -D warnings clean. No changes needed. Proceeding to /review.'
  timestamp: 2026-06-15T17:17:09.268510+00:00
- actor: claude-code
  id: 01kv656vz1x56xqb5mm5zpp8hz
  text: '/review COMPLETE (15/15 sub-tasks, 0 failed): 0 blockers, 2 warnings, 2 nits — none holding. Warnings (×2): introduce TaskId/ContentHash newtypes on get/put — assessed FALSE POSITIVE in context (the entire kanban crate uses bare task_id: &str — task_helpers.rs, clipboard_commands.rs, paste_handlers; no such newtype exists anywhere; adopting in one file would invent an inconsistent pattern). Nits: EmbeddingCache lacks Debug impl; dim-mismatch test hardcodes 8 — optional polish. No duplicate-definition/compile blockers (consistent with crate compiling + 1450 tests passing). embedding_cache 8/8 pass. Moved to done.'
  timestamp: 2026-06-15T17:28:00.737740+00:00
depends_on:
- 01KTC7Y50PEM427HQ79NW52WY4
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffab80
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

Storage location: derive from the board dir. `KanbanContext::root()` returns the `.kanban` directory (see `crates/swissarmyhammer-kanban/src/context.rs`). Put the sidecar at `<root>/search-cache.sqlite3` (sibling of `board.yaml`/`tasks/`). Add a `KanbanContext` path helper `pub fn search_cache_path(&self) -> PathBuf { self.root.join("search-cache.sqlite3") }` next to the other path helpers (`board_path`, `tasks_dir`, …).

Schema (created on open if absent):
- `embeddings(task_id TEXT NOT NULL, content_hash TEXT NOT NULL, vector BLOB NOT NULL, PRIMARY KEY(task_id, content_hash))`.
- `meta(key TEXT PRIMARY KEY, value TEXT)` storing the embedder model name (`Embedder::model_name`) and dimension. On open, if the stored model name or dim differs from the current embedder's, DROP/clear `embeddings` (self-healing invalidation) and rewrite the meta row.

API (sync rusqlite behind a small struct; embedding calls are async via the `Embedder`):
- `pub struct EmbeddingCache { conn: rusqlite::Connection }` with `open(path, model_name, dim) -> Result<Self>` — creates the schema in WAL mode (`PRAGMA journal_mode=WAL`, so concurrent readers — GUI, MCP server, CLI — don't block the writer) and runs model/dim invalidation. `open` MUST create the file and schema when the path does not exist — this is the cold-start / fresh-clone rebuild path.
- `pub fn get(&self, task_id: &str, content_hash: &str) -> Option<Vec<f32>>` (cache hit -> deserialize blob).
- `pub fn put(&self, task_id: &str, content_hash: &str, vector: &[f32])` — serialize + upsert, AND delete any other rows for the same `task_id` (different `content_hash`) so the cache holds at most ONE current vector per task. Without this, every task edit leaves a dead row and the cache grows unbounded.
- Single canonical embed-text composition lives HERE so hashing and embedding can never drift: `pub fn task_embedding_text(title: &str, description: &str) -> String` (embed text = `title` + "\n" + `description`). Tags are NOT embedded — they are a lexical-only Doc field in the search-op card, so a tag-only edit must not change the hash or force a re-embed. `pub fn content_hash(text: &str) -> String` hashes that same string with a stable hash (xxh3 / sha2; even a machine-local non-portable hash like `DefaultHasher` is acceptable since the cache is gitignored and rebuilt per machine). The search-op card MUST call `task_embedding_text` once and use its output for BOTH `content_hash` and the embedder input.

Lazy-fill belongs to the search-op card (it owns the `Embedder` and the corpus); THIS card delivers the store + invalidation + helpers, fully unit/integration tested without needing a real model (insert/get/put/hash and the model/dim-mismatch invalidation can all be tested with synthetic vectors and fake model-name/dim args).

### Gitignore (the cache is derived data, never committed)
The cache lives INSIDE `.kanban/`, which is only PARTIALLY tracked — the task `.md`/`.jsonl` files there are committed source, so we must NOT blanket-`*`-ignore `.kanban/` the way `.code-context/.gitignore` does. Instead, extend the existing `.kanban/.gitignore` writer in `crates/swissarmyhammer-kanban/src/board/init.rs` (it currently writes `mcp.log`) to also ignore the cache and its SQLite WAL sidecars:
- `search-cache.sqlite3`
- `search-cache.sqlite3-wal`
- `search-cache.sqlite3-shm`

(A single `search-cache.sqlite3*` glob is acceptable.) Make the writer idempotent — if `.kanban/.gitignore` already exists, ensure the lines are present without duplicating them (existing boards must gain the entries on next init/open, not only freshly-created ones). Do NOT rely on the stray `search.db` pattern in the root `.gitignore`; ignore explicitly in the board's own ignore file so the guarantee travels with the board dir.

## Acceptance Criteria
- [x] `crates/swissarmyhammer-kanban/Cargo.toml` gains `rusqlite`, `swissarmyhammer-search`, `swissarmyhammer-embedding` deps; `cargo build -p swissarmyhammer-kanban` compiles.
- [x] `EmbeddingCache::open` creates the sidecar at `<root>/search-cache.sqlite3` in WAL mode with the `embeddings` + `meta` tables.
- [x] `put` then `get` for the same `(task_id, content_hash)` returns the exact vector (round-trips through the search-crate blob helpers).
- [x] `get` for a missing `(task_id, content_hash)` returns `None`.
- [x] `put` keeps at most one row per `task_id`: after putting a new `content_hash` for a task, prior rows for that `task_id` are gone.
- [x] `task_embedding_text(title, description)` is the single composition used for both hashing and embedding; a tag-only change does not alter it (tags are not part of the embed text).
- [x] Opening the cache with a different model name OR a different dim than the stored meta clears `embeddings` (self-healing) and updates `meta`.
- [x] `KanbanContext::search_cache_path()` returns `<root>/search-cache.sqlite3`.
- [x] **Cold start:** `EmbeddingCache::open` against a path that does NOT exist creates a fully usable cache (schema present, `put`/`get` work) — no pre-existing file required. This is the cross-machine / fresh-clone rebuild guarantee at the store layer.
- [x] **Gitignore:** after board init (and on open of a pre-existing board), `.kanban/.gitignore` contains `search-cache.sqlite3` (+ `-wal`/`-shm`, or the glob), the entries are not duplicated on repeat, and `git check-ignore` would match the cache file. `.kanban/`'s tracked task files remain un-ignored.

## Tests
- [x] Unit/integration tests in `embedding_cache.rs` `#[cfg(test)] mod tests` using a `TempDir`: put/get round-trip; get-miss returns None; `put` prunes prior rows for the same task_id (only the latest hash survives); `task_embedding_text` excludes tags / `content_hash` stable for equal input and differs for different input; model-name-mismatch invalidation clears rows; dim-mismatch invalidation clears rows; reopening with the SAME model/dim preserves rows.
- [x] Cold-start test: `open` on a non-existent path inside a fresh `TempDir` succeeds and supports `put`/`get` (proves the rebuild-on-checkout path at the store layer).
- [x] Gitignore test (in `board/init.rs` tests or `embedding_cache.rs`): after writing/refreshing `.kanban/.gitignore`, assert it contains the `search-cache.sqlite3` (+ sidecar) patterns, that running the writer twice does not duplicate lines, and that the committed task files are still not ignored.
- [x] `cargo test -p swissarmyhammer-kanban embedding_cache` and the gitignore test pass (all new tests green, no real model required).

## Workflow
- Use `/tdd` — write failing tests first, then implement to pass.

## Review Findings (2026-06-15 12:17)

### Warnings
- [ ] `crates/swissarmyhammer-kanban/src/task/embedding_cache.rs` `get` — Adjacent parameters `task_id` and `content_hash` are both `&str` with different semantic meanings; engine suggests TaskId/ContentHash newtypes. ASSESSED — false positive in context: the entire kanban crate uses bare `task_id: &str` (task_helpers.rs, clipboard_commands.rs, paste_handlers); there is no TaskId/ContentHash newtype anywhere. Introducing them only here would invent a pattern inconsistent with the crate. Not a blocker for this card. (Optional crate-wide refactor if newtypes are ever adopted board-wide.)
- [ ] `crates/swissarmyhammer-kanban/src/task/embedding_cache.rs` `put` — Same newtype suggestion as above (`task_id`/`content_hash`). Same assessment: matches crate convention, not a blocker.

### Nits
- [ ] `crates/swissarmyhammer-kanban/src/task/embedding_cache.rs` — `EmbeddingCache` (public type) does not implement `Debug`. Optional polish.
- [ ] `crates/swissarmyhammer-kanban/src/task/embedding_cache.rs` (dim-mismatch test) — hardcoded dimension `8` could be a named constant. Optional test-readability polish.

_Verdict: 0 blockers, 2 warnings (both the same newtype suggestion — false positive vs. the crate's established `&str` convention), 2 nits (optional polish). No genuine correctness blocker or substantive warning in this card's deliverables. Engine run was COMPLETE (15 attempted, 0 failed). All 8 `embedding_cache` tests pass. Moved to `done`._