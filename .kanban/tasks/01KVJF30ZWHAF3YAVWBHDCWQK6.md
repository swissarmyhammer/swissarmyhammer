---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvjfwcenk1kz0zz3ffp967w1
  text: '/finish picked up this task (single-task mode). Iteration 1: dispatching /implement. This is a well-specified TDD bug: watcher Created events use UPDATE-only SQL that matches 0 rows for brand-new files, so files created after startup never get indexed. RED test spec provided in description.'
  timestamp: 2026-06-20T12:25:24.693335+00:00
- actor: claude-code
  id: 01kvjg8xvp38ydxygrej0cayw6
  text: |-
    Implemented TDD fix.

    RED: Added unit test `watcher_indexes_file_created_after_startup` in tools watcher.rs tests. Confirmed FAIL on current code — assertion "new file created after startup must be inserted into indexed_files" failed (left:0, right:1), exactly the documented bug.

    GREEN: Changed `FanoutWatcher::notify` (code-context crate watcher.rs) Created/Modified arm from UPDATE-only to an UPSERT: `INSERT INTO indexed_files (...) VALUES (?, X'', 0, ?now, 0, 0) ON CONFLICT(file_path) DO UPDATE SET ts_indexed=0, lsp_indexed=0`. A brand-new row-less file is now inserted (placeholder hash/size; the dirty-set indexer reads the file fresh and computes the real values), and existing rows still get flags cleared. File enters the `ts_indexed=0` dirty set the incremental indexer drains.

    Also updated the misleading existing test `test_create_event_on_unknown_file_does_not_error` -> `test_create_event_on_unknown_file_inserts_dirty_row` (now asserts 1 row inserted + dirty), and added a crate-level guard `test_created_event_inserts_dirty_row_for_new_file` next to the SQL.

    Test status: target test PASS. All 5 code-context watcher tests PASS, all 641 code-context lib tests PASS, all 109 tools code_context tests PASS (incl. e2e). cargo fmt + clippy clean.

    Secondary rust-analyzer readiness issue is NOT addressed here per task instructions — track separately.
  timestamp: 2026-06-20T12:32:15.734476+00:00
- actor: claude-code
  id: 01kvjgg86ey4vt995mw3gadgpr
  text: |-
    Adversarial double-check verdict: PASS. UPSERT is correct/complete, X'' is a valid non-NULL BLOB, no stale old-SQL tests remain, scope minimal (secondary rust-analyzer issue untouched). Two bounded/self-healing side effects noted, neither a blocker: (1) placeholder content_hash=X'' persists until the next startup_cleanup re-indexes the file once and writes the real hash (pre-existing indexer characteristic — the dirty-set indexer never writes content_hash back); (2) a [Deleted(x), Modified(x)] batch can briefly INSERT a phantom row for an off-disk file, which the indexer handles gracefully (ts_indexed=1, no retry loop) and the next startup_cleanup DELETEs. Proceeding to review.

    Note: working tree also contains three unrelated pre-existing modified files (mirdan/src/install.rs, tool_registry.rs, files/mod.rs) NOT part of this fix — do not bundle them into the watcher-fix commit.
  timestamp: 2026-06-20T12:36:15.694256+00:00
- actor: claude-code
  id: 01kvjghabhtg4t2tffvmcfqa4x
  text: 'Implementation landed in review. Fix: turned the watcher''s UPDATE-only SQL in FanoutWatcher::notify (crates/swissarmyhammer-code-context/src/watcher.rs) into an UPSERT (INSERT ... ON CONFLICT(file_path) DO UPDATE SET ts_indexed=0, lsp_indexed=0) so a row-less new file is inserted with ts_indexed=0 and enters the dirty set. Added RED test watcher_indexes_file_created_after_startup (RED→GREEN confirmed), renamed the misleading test_create_event_on_unknown_file_does_not_error → ..._inserts_dirty_row, plus a crate-level guard test. Secondary rust-analyzer readiness issue left untouched per task. NOTE: working tree has 3 unrelated pre-existing modified files (mirdan/src/install.rs, tool_registry.rs, files/mod.rs) that must NOT be bundled into this commit. Iteration 1: dispatching /test.'
  timestamp: 2026-06-20T12:36:50.673698+00:00
- actor: claude-code
  id: 01kvjgtt6gm1je8msjmntrr9zs
  text: '/test green for the fix: 1976/1977 pass across swissarmyhammer-code-context + swissarmyhammer-tools; clippy --all-targets clean. The new regression tests all pass (watcher_indexes_file_created_after_startup, test_create_event_on_unknown_file_inserts_dirty_row, test_created_event_inserts_dirty_row_for_new_file) and the e2e test_mcp_detects_new_files passes. The 1 failure (per_client_tool_composition::claude_client_gets_shared_plus_shell_not_agent_tools — "files tool must NOT be advertised") is pre-existing and unrelated, driven by the 3 untouched working-tree files (files/mod.rs, tools/mod.rs, mirdan/install.rs). Iteration 1: dispatching /review.'
  timestamp: 2026-06-20T12:42:01.808814+00:00
- actor: claude-code
  id: 01kvjha8etp2535fn0jwp5vkwx
  text: Review scoped to the watcher fix only (crates/swissarmyhammer-code-context/src/watcher.rs + crates/swissarmyhammer-tools/src/mcp/tools/code_context/watcher.rs). The 3 unrelated pre-existing modified files (mirdan/src/install.rs, tools/.../mcp/tools/mod.rs, tools/.../files/mod.rs) were excluded from scope per the review request. Cited line numbers drifted slightly; the substantive findings stand.
  timestamp: 2026-06-20T12:50:27.930068+00:00
- actor: claude-code
  id: 01kvjhbwgz467wbxta86940233
  text: 'Iteration 1 review: FINDINGS (2 blockers, 3 warnings) — core UPSERT fix not flagged. Blockers: new test should reuse existing file_exists()/get_ts_indexed() helpers instead of inlined COUNT(*)/ts_indexed queries. Warnings: FileEvent::path() should return &Path not &PathBuf; run_watcher Ok-branch could be extracted; watcher fns could take impl AsRef<Path>. Reviewer flagged that engine line cites drifted — locate the inlined queries inside the new test. Iteration 2: dispatching /implement to work the findings.'
  timestamp: 2026-06-20T12:51:21.247776+00:00
- actor: claude-code
  id: 01kvjhmc7vvg7gepf0vm8b6y70
  text: |-
    Iteration 2: worked all 5 review findings. (No GFM checklist existed in the task body/comments — findings were the prose list in the iteration-1 review comment; recording resolution here.)

    Blockers (test watcher_indexes_file_created_after_startup in tools/.../code_context/watcher.rs):
    - [x] 1. Replaced inlined COUNT(*) existence check with assert!(file_exists(&conn, "src/created_after_start.rs")) — reusing the existing test-module helper (fn file_exists(&Connection, &str) -> bool).
    - [x] 2. Replaced inlined ts_indexed query with assert_eq!(get_ts_indexed(&conn, "src/created_after_start.rs"), Some(1)) — reusing fn get_ts_indexed(&Connection, &str) -> Option<i64>. (Chunks assertion left inlined — no existing helper duplicates it.)

    Warnings:
    - [x] 3. FileEvent::path() (code-context/src/watcher.rs) now returns &Path (was &PathBuf), via p.as_path() in the match arm; added Path to the use import. Both callers (event.path().to_string_lossy() here, and {:?} debug-format in tools/watcher.rs) work unchanged on &Path.
    - [x] 4. Extracted the run_watcher Ok(debounced_events) branch into async fn process_ok_events(db, ws_root, fanout, debounced_events: &[async_watcher::DebouncedEvent]); run_watcher's loop now just calls it. run_watcher dropped from ~60 to ~20 lines.
    - [x] 5. start_code_context_watcher and run_watcher now take impl AsRef<Path> instead of concrete PathBuf (start_ converts to owned PathBuf before the 'static spawn; run_ binds .as_ref() locally). Sole caller (server.rs, passes a PathBuf) needs no change.

    Verification: cargo fmt clean. cargo clippy --all-targets -- -D warnings clean on both crates (exit 0, 0 warnings). cargo nextest run -p swissarmyhammer-code-context -p swissarmyhammer-tools: 1976/1977 passed; the 3 regression tests (watcher_indexes_file_created_after_startup, test_create_event_on_unknown_file_inserts_dirty_row, test_created_event_inserts_dirty_row_for_new_file) and e2e test_mcp_detects_new_files all PASS. The 1 failure is the documented pre-existing per_client_tool_composition::claude_client_gets_shared_plus_shell_not_agent_tools (driven by the 3 untouched files mirdan/install.rs, tools/mod.rs, files/mod.rs) — left alone per task constraints. Task stays in review.
  timestamp: 2026-06-20T12:55:59.483380+00:00
- actor: claude-code
  id: 01kvjhn71mq7dqq5c1ptkhnp3b
  text: 'Iteration 2 implement: all 5 findings resolved. Blockers 1&2: test now reuses file_exists()/get_ts_indexed() helpers. W3: FileEvent::path() returns &Path. W4: extracted process_ok_events() from run_watcher. W5: watcher fns take impl AsRef<Path>. Tests 1976/1977 (only pre-existing unrelated failure), clippy --all-targets clean, fmt clean. Iteration 2: dispatching /review to re-verify and move to done.'
  timestamp: 2026-06-20T12:56:26.932177+00:00
- actor: claude-code
  id: 01kvjj4hj9n1vbx7zx0szfwqky
  text: 'Iteration 2 re-review: prior 5 findings ALL cleared (not re-raised). 4 new smaller items surfaced (2 warnings, 2 nits) — none blockers. Triage decision: (1) add # Errors doc to notify() — DO; (3) name the 1s debounce constant — DO; (4) align test doc re: index_discovered_files_with_embedder vs production index_discovered_files_async — DO (doc clarification). DECLINING (2) wrap rusqlite::Error in a new thiserror WatcherError type — that is a public-API refactor beyond this bug''s scope; the finish skill forbids bonus refactoring and it would ripple to callers and likely spawn further churn. Iteration 3: dispatching /implement for items 1,3,4 only.'
  timestamp: 2026-06-20T13:04:49.225392+00:00
- actor: claude-code
  id: 01kvjjbbpva749rd42tys1yzws
  text: |-
    Iteration 3 implement: applied review findings 1, 3, 4 (item 2 DECLINED per triage — NOT applied).

    (1) # Errors doc — crates/swissarmyhammer-code-context/src/watcher.rs: added an `# Errors` rustdoc section to FanoutWatcher::notify documenting it returns rusqlite::Error when the DELETE / INSERT ... ON CONFLICT statement fails (missing schema, locked DB, I/O error). Return type unchanged (Result<usize, rusqlite::Error>).

    (3) Named debounce const — crates/swissarmyhammer-tools/src/mcp/tools/code_context/watcher.rs: extracted the hardcoded Duration::from_secs(1) (in AsyncDebouncer::new_with_channel) to module-level `const DEBOUNCE_TIMEOUT: Duration = Duration::from_secs(1);` with a doc comment; call site and the start_code_context_watcher doc now reference DEBOUNCE_TIMEOUT.

    (4) Test doc accuracy — same tools file: rewrote the doc of watcher_indexes_file_created_after_startup. It no longer claims "the exact production sequence"; it now explains the one deliberate divergence — production process_ok_events calls index_discovered_files_async (which builds the default embedder then delegates), while the test calls index_discovered_files_with_embedder directly with embedder=None for determinism/speed. The dirty-set insert/index path exercised is identical; embedding is orthogonal to the bug. Doc-only; no production change.

    (2) NOT done: wrapping rusqlite::Error in a thiserror WatcherError type — explicitly declined as out-of-scope public-API refactor.

    Verification: cargo fmt clean; cargo clippy -p swissarmyhammer-code-context -p swissarmyhammer-tools --all-targets -- -D warnings clean (exit 0); cargo nextest run -p swissarmyhammer-code-context -p swissarmyhammer-tools = 1976/1977 passed. The 3 watcher regression tests + e2e test_mcp_detects_new_files all PASS. The lone failure is the documented pre-existing unrelated per_client_tool_composition::claude_client_gets_shared_plus_shell_not_agent_tools (driven by the 3 untouched files mirdan/install.rs, tools/mod.rs, files/mod.rs) — left alone per task constraints.
  timestamp: 2026-06-20T13:08:32.603370+00:00
- actor: claude-code
  id: 01kvjjfzcx82tgfnnj3jftm9a8
  text: 'Iteration 3 implement: items 1 (# Errors doc on notify), 3 (named DEBOUNCE_TIMEOUT const), 4 (test doc accuracy re: with_embedder vs _async) done. Item 2 (thiserror wrap) correctly NOT done. Adversarial double-check PASS. Tests 1976/1977 (pre-existing unrelated failure only), clippy --all-targets clean, fmt clean. Iteration 3: dispatching final /review. Note: findings have decayed blockers→warnings→nits and the core UPSERT fix was never flagged across two passes; this is the convergence pass.'
  timestamp: 2026-06-20T13:11:03.837119+00:00
- actor: claude-code
  id: 01kvjk0sah7xjgmtkzad4pt7f1
  text: 'DONE. Final convergence review: core UPSERT fix passed its 3rd review pass unflagged; remaining items were test-only style nits + one re-raise of an already-resolved finding (and the declined thiserror wrap), none blocking. Task moved to done (completed 2026-06-20T13:18:36Z). Local rollback-point commit 7dbcaa745 created (NOT pushed) with exactly the 2 watcher-fix files + this task''s 2 .kanban files. Leftover non-blocking test-cleanup nits (collapse get_ts_indexed/get_lsp_indexed helpers; name 1024 / symbol-kind 12 literals) noted, safe to sweep later. The secondary rust-analyzer readiness issue (queried ~1s after spawn before warmup, returns 0 symbols, never retried) was deliberately left out of scope per the task and should be tracked as its own card if desired.'
  timestamp: 2026-06-20T13:20:14.673277+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffce80
project: diagnostics
title: 'code-context watcher never indexes files created after startup (calcutron run: LSP/index empty)'
---
# code-context: live file watcher never indexes files created after startup

Investigation of the calcutron demo run (`/Users/wballard/github/swissarmyhammer/calcutron`, demo.sh `/finish` build, Jun 19 2026). The whole generated Rust source is missing from the index; every `get_symbol`/`get_blastradius` fell back to live tree-sitter.

## Confirmed behavior

**Leader election worked.** Exactly one leader (`sah serve` PID 9852, started Jun 19 14:18:09, the `/finish` parent) held the flock for the whole run; the other 314 processes were correctly followers. Verified live: the lock `/var/folders/.../T/code-context-ts-bc3927cb1cd02e5607b40c650bbd4c3b.lock` records PID 9852 and the flock is still held. (The earlier "0 leaders" reading was an artifact: the leader's log `mcp.9852.log` is still open, so a Jun-19-mtime filter skipped it. Logs are UTC; file mtimes local CDT.)

**The leader's live watcher cannot index files created after the startup scan.** Across the entire 3-hour run the leader logged, repeatedly:
```
19:19:38  16 file change(s) detected, marking dirty
19:19:38  code-context watcher: 0 dirty, 0 deleted, 0 errors
19:19:39  no dirty files to index
```
On-disk index DB is frozen at startup: `indexed_files`=1 (`demo.sh`), `ts_chunks`=2 (demo.sh), `lsp_symbols`=2 (both `ts:`-prefixed = tree-sitter), `lsp_call_edges`=0. The 12 generated `.rs` files were indexed briefly at startup, **deleted** at 19:19:31 (`12 deleted`), regenerated by `/finish`, and never re-indexed.

## Root cause

The only production code that discovers + INSERTs files is `startup_cleanup` (`crates/swissarmyhammer-code-context/src/cleanup.rs`), called **only** at `open_as_leader` and `try_promote` — once each, never on a timer. The live watcher does NOT call it. Instead:

- `FanoutWatcher::notify` (`crates/swissarmyhammer-code-context/src/watcher.rs`) handles Created identically to Modified with **UPDATE-only** SQL: `UPDATE indexed_files SET ts_indexed=0, lsp_indexed=0 WHERE file_path=?1` — **no INSERT**. A brand-new file (no row) matches **0 rows** → `dirty_count=0`.
- `index_discovered_files_with_embedder` (`crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs`) only selects `WHERE ts_indexed = 0` from existing rows; it does **not** walk the filesystem (its own doc: "the dirty-file query below is 'discovery'").

| File state during a run | Indexed? | Why |
|---|---|---|
| Existing/known file **modified** | ✅ | UPDATE matches its row → `ts_indexed=0` → reindexed |
| **New** file created | ❌ | UPDATE matches 0 rows, no INSERT |
| Deleted | ✅ | DELETE removes row (cascade) |
| Deleted then **re-created** | ❌ | row gone; re-create can't UPDATE it back |

A long-lived leader stays frozen on its startup snapshot for its entire life.

## Why this shipped green — the test gap

There is **no test** that starts the real watcher, creates a new file, and asserts it gets indexed.
- `code_context_mcp_e2e_test.rs::test_mcp_detects_new_files` creates a file after startup but then calls the **`rebuild index`** op (a full rescan) — it bypasses the watcher entirely. Nothing auto-calls `rebuild index` in a real session.
- The `watcher.rs` unit tests all **pre-insert** the file row, so the no-row case never flows through indexing. `test_create_event_on_unknown_file_does_not_error` documents the broken result (0 rows) as acceptable.

## TDD — RED test to add (must FAIL on current code)

Isolated, deterministic, fast (`embedder = None`, no model load, no real-FS-watch flake). Drives the EXACT production sequence the watcher runs (`process_file_events` then the dirty-set indexer) against a real on-disk workspace, with the new file created AFTER initial indexing. Place in `crates/swissarmyhammer-tools/src/mcp/tools/code_context/watcher.rs` tests (has `pub(crate)` access to both fns).

```rust
#[tokio::test]
async fn watcher_indexes_file_created_after_startup() {
    use std::sync::{Arc, Mutex};
    use swissarmyhammer_code_context::{FanoutWatcher, FileEvent};
    use swissarmyhammer_code_context::db::{configure_connection, create_schema};

    // --- workspace with ONE file already indexed at "startup" ---
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/existing.rs"), "pub fn a() {}").unwrap();

    let conn = Connection::open(dir.path().join("index.db")).unwrap();
    configure_connection(&conn).unwrap();
    create_schema(&conn).unwrap();
    // existing.rs is already in the index (as startup_cleanup would have left it)
    conn.execute(
        "INSERT INTO indexed_files (file_path, content_hash, file_size, last_seen_at, ts_indexed, lsp_indexed)
         VALUES ('src/existing.rs', X'00', 13, 1000, 1, 1)", []).unwrap();
    let db: swissarmyhammer_code_context::SharedDb = Arc::new(Mutex::new(conn));

    // --- a NEW file is created AFTER startup (what /finish does) ---
    std::fs::write(dir.path().join("src/created_after_start.rs"),
        "pub fn brand_new() -> i32 { 42 }").unwrap();

    // --- exactly what run_watcher does on the resulting notify event ---
    let fanout = FanoutWatcher::new();
    let events = vec![FileEvent::Created(std::path::PathBuf::from("src/created_after_start.rs"))];
    {
        let conn = db.lock().unwrap();
        super::process_file_events(&conn, &fanout, &events);
    }
    super::super::index_discovered_files_with_embedder(
        dir.path(), Arc::clone(&db), None,
        swissarmyhammer_code_context::noop_reporter(),
    ).await;

    // --- ASSERT the new file is indexed (FAILS today) ---
    let conn = db.lock().unwrap();
    let in_files: i64 = conn.query_row(
        "SELECT COUNT(*) FROM indexed_files WHERE file_path = 'src/created_after_start.rs'",
        [], |r| r.get(0)).unwrap();
    assert_eq!(in_files, 1, "new file created after startup must be inserted into indexed_files");
    let ts_indexed: i64 = conn.query_row(
        "SELECT ts_indexed FROM indexed_files WHERE file_path = 'src/created_after_start.rs'",
        [], |r| r.get(0)).unwrap();
    assert_eq!(ts_indexed, 1, "new file must be tree-sitter indexed");
    let chunks: i64 = conn.query_row(
        "SELECT COUNT(*) FROM ts_chunks WHERE file_path = 'src/created_after_start.rs'",
        [], |r| r.get(0)).unwrap();
    assert!(chunks > 0, "new file must produce ts_chunks");
}
```
(Verify exact import paths / `pub(crate)` visibility of `index_discovered_files_with_embedder` and `noop_reporter` when wiring it up; adjust `super::` depth as needed.)

Optional higher-fidelity follow-up test (separate, in `tests/`): drive `start_code_context_watcher` with a real `AsyncDebouncer`, `fs::write` a new file, poll up to ~5s, assert it lands in `indexed_files`. Keep it out of the unit suite (slow/FS-event) per the <10s unit-test rule.

## GREEN — fix direction

On a `Created` event (or any event whose path exists on disk with no row), **INSERT** an `indexed_files` row (`ts_indexed=0, lsp_indexed=0`) so it enters the dirty set — or have the watcher run the same `startup_cleanup` reconcile instead of UPDATE-only. Then the existing dirty-set indexer picks it up. Keep the deterministic RED test as the regression guard.

## Secondary issue

rust-analyzer was queried ~1s after spawn (19:18:11→19:18:12), before warmup, returned 0 symbols/edges, persisted them, never retried — so LSP data stays empty even for indexed files. Needs a readiness gate (`$/progress`/workDoneProgress end) or retry-on-empty, then re-drive `lsp_indexed=0`. Track separately if preferred.

## Anchors
- Leader log: `calcutron/.sah/mcp.9852.log` (grep `marking dirty` / `no dirty files to index`)
- Index DB: `calcutron/.code-context/index.db`
- Code: `swissarmyhammer-code-context/src/watcher.rs` (`FanoutWatcher::notify`), `.../cleanup.rs` (`startup_cleanup`), `swissarmyhammer-tools/src/mcp/tools/code_context/watcher.rs` (`process_file_events`, `run_watcher`), `.../mod.rs` (`index_discovered_files_with_embedder`, `WHERE ts_indexed = 0`)
- Existing misleading test: `swissarmyhammer-tools/tests/code_context_mcp_e2e_test.rs::test_mcp_detects_new_files` #bug #code-context #indexer #lsp-live

## Review Findings (2026-06-20 07:59)

Re-review scoped to the two task files only (`crates/swissarmyhammer-code-context/src/watcher.rs`, `crates/swissarmyhammer-tools/src/mcp/tools/code_context/watcher.rs`). Iteration-1's 5 findings are resolved (not re-raised). New findings below.

### Warnings
- [ ] `crates/swissarmyhammer-code-context/src/watcher.rs` — `notify()` (the function returning `Result`) lacks error documentation. The doc comment describes behavior but not failure modes, leaving callers uncertain what can fail. Add an `# Errors` section, e.g., `/// # Errors\n/// Returns a database error if the SQL operation fails.`.
- [ ] `crates/swissarmyhammer-code-context/src/watcher.rs` — Public API leaks the concrete `rusqlite::Error` third-party type. Returning `rusqlite::Error` directly couples the public API to rusqlite and prevents downstream decoupling/custom handling. Define a custom error with `thiserror`, e.g., `#[derive(Debug, thiserror::Error)] pub enum WatcherError { #[error(\"database error\")] Database(#[from] rusqlite::Error) }`, and return `Result<usize, WatcherError>`.

### Nits
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/watcher.rs` — Hardcoded 1-second debounce timeout should be a named module-level constant for clarity/configurability: `const DEBOUNCE_DELAY: Duration = Duration::from_secs(1);`, used in the `AsyncDebouncer::new_with_channel()` call.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/watcher.rs` — Regression test doc claims to drive 'the exact production sequence' but the test calls `super::super::index_discovered_files_with_embedder` while production `process_ok_events` calls `super::index_discovered_files_async` — different functions. Either align the test to call the same fn as production (`index_discovered_files_async`), align production to the test, or update the test doc to explain why the divergence is intentional.