---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvjtw4kkedz4jbaezzr3hmvk
  text: '/finish picked up (single-task mode). Iteration 1: dispatching /implement. Actual-fix task: give the code-context leader a periodic startup_cleanup reconcile (correctness floor) so a long-lived leader self-heals; keep ^hdcwqk6''s UPSERT as the event fast-path. Diagnosed live in calcutron (leader PID 9852 frozen on Jun-19 snapshot, "0 dirty / no dirty files to index"). RED test = mutate FS without a watcher event → reconcile → assert new/recreated files indexed.'
  timestamp: 2026-06-20T15:37:30.995977+00:00
- actor: claude-code
  id: 01kvjvek2145mz0xezwrbgwcaj
  text: |-
    Implemented. Entrypoint added: `reconcile_workspace_with_embedder(workspace_root, &db, embedder, reporter)` in crates/swissarmyhammer-tools/src/mcp/tools/code_context/watcher.rs — locks the shared conn, runs `startup_cleanup` (FS-walk reconcile), drops the guard, then drains the dirty set via `index_discovered_files_with_embedder` (WHERE ts_indexed = 0). Prod wrapper `reconcile_workspace` builds the default embedder; `run_periodic_reconcile` drives it on a `RECONCILE_INTERVAL` (named const = 5min, doc-commented) tokio interval timer, skipping the immediate first tick since startup already ran startup_cleanup.

    Leader gating: the timer is spawned inside `spawn_ts_and_watcher_workers` (server.rs), which is only reachable on the leader — via `start_workers_if_leader` (the Some/leader branch of do_initialize_code_context) and `start_indexing_workers_after_promotion` (post-promotion cold re-spawn). The follower (None) branch spawns no workers. So leader-only holds by construction, same as the watcher spawned right above it. ^hdcwqk6's watcher UPSERT fast-path is untouched.

    RED→GREEN: new unit test `periodic_reconcile_indexes_files_changed_without_a_watcher_event` (embedder=None, 0.052s). RED failed to compile (reconcile_workspace_with_embedder not found); GREEN passes. It writes a brand-new file AND delete+recreates an existing file with NO watcher event, calls the reconcile entrypoint directly, asserts both land ts_indexed=1 with ts_chunks > 0.

    Tests: `cargo nextest run -p swissarmyhammer-code-context -p swissarmyhammer-tools` = 1977 passed, 1 failed. The single failure is the pre-existing unrelated `per_client_tool_composition::claude_client_gets_shared_plus_shell_not_agent_tools` the task said to leave alone (in a file I never touched). clippy --all-targets -D warnings clean on both crates; cargo fmt clean.
  timestamp: 2026-06-20T15:47:35.617892+00:00
- actor: claude-code
  id: 01kvjvkq8y2ayqraa2zr2ys25c
  text: 'Iteration 1 implement landed in review. Added reconcile_workspace_with_embedder() in code_context/watcher.rs (locks shared conn → startup_cleanup FS-walk → drains dirty set via index_discovered_files_with_embedder); run_periodic_reconcile drives it on a timer; const RECONCILE_INTERVAL = 5min (documented). Timer spawned in server.rs spawn_ts_and_watcher_workers right after the watcher — leader-only by construction (only reachable from start_workers_if_leader and start_indexing_workers_after_promotion; follower arm spawns no workers). ^hdcwqk6 UPSERT fast-path untouched. RED→GREEN: periodic_reconcile_indexes_files_changed_without_a_watcher_event (embedder=None, 0.052s) — writes a new file + delete/recreates an existing one with NO watcher event, calls reconcile directly, asserts both ts_indexed=1 & ts_chunks>0; RED failed to compile, GREEN passes. nextest 1977 pass / 1 pre-existing unrelated fail; clippy+fmt clean; double-check PASS. Iteration 1: dispatching /review.'
  timestamp: 2026-06-20T15:50:23.774811+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffcf80
project: diagnostics
title: 'code-context: long-lived leader never re-reconciles workspace (index frozen on startup snapshot; only manual rebuild/restart recovers)'
---
# code-context: a long-lived leader never re-reconciles the workspace

The index goes stale and **stays** stale for the entire life of a code-context leader, because nothing ever re-walks the filesystem after the leader starts. This is the durable fix: make the leader self-heal so a correct index never depends on a restart or a hand-run `rebuild index`.

## Root cause

`startup_cleanup` (`crates/swissarmyhammer-code-context/src/cleanup.rs`) is the **only** production code that walks the filesystem and reconciles `indexed_files` against what's actually on disk. It runs **once**, at `open_as_leader` / `try_promote`, and never again. After that the index is maintained purely by the live watcher's per-event path — which has no correctness floor:

- The watcher depends on a debounced FS event firing for every change. Any miss — event storms, editors that write via rename/replace, files materialized before the watcher attaches, bulk regeneration — leaves rows the event path never revisits.
- ^hdcwqk6 (DONE) fixed the watcher's `Created`/`Modified` SQL (UPDATE-only → UPSERT) so a row-less new file enters the dirty set. That repairs the *event fast-path*, but it does nothing for changes whose event never arrives, and nothing for a leader running on stale state.
- A leader that lives for hours/days therefore drifts permanently with no automatic recovery.

## Live evidence (calcutron, fresh run Jun 20 2026)

`.code-context/index.db` frozen at the leader's Jun-19 startup snapshot: `indexed_files=1` (only `demo.sh`), `ts_chunks=2`, `lsp_symbols=2` (both `ts:` tree-sitter), `lsp_call_edges=0` — while `src/`+`tests/` hold 7 real `.rs` files (`repl.rs error.rs eval.rs main.rs ast.rs parser.rs tests/cli.rs`) that are entirely absent. The election lock `…/T/code-context-ts-bc3927cb….lock` holds PID 9852, a `sah serve` started Jun 19 14:18 whose parent `claude` (PID 8273) is still alive ~19h later — so leadership is legitimate, the leader is just long-lived and frozen. Live on that leader today:
```
2026-06-20T14:31:21Z  INFO code-context: 2 file change(s) detected, marking dirty
2026-06-20T14:31:21Z  INFO code-context watcher: 0 dirty, 0 deleted, 0 errors
2026-06-20T14:31:22Z  INFO code-context: no dirty files to index
```

## Fix direction

Give the leader a periodic correctness floor, keeping ^hdcwqk6's UPSERT as the low-latency fast-path:

- Run `startup_cleanup` (the FS-walk reconcile) **periodically** on the leader on a timer, not just at open/promote, so a long-lived leader converges on disk truth. It already diffs by content-hash/mtime and only marks genuinely-changed rows dirty, so the steady-state cost is bounded.
- And/or expose a leader-side reconcile entrypoint that a follower's `rebuild index` / `get status` request can trigger over the existing election socket, so a reader can force a re-scan without restarting the server.

## Test approach (deterministic, no model load — embedder=None, <10s unit budget)

Open a workspace as leader and index it, then mutate the filesystem **without delivering a watcher event** (write a new file; delete+recreate an existing one), invoke the periodic-reconcile entrypoint directly, and assert the new/recreated files land in `indexed_files` (`ts_indexed=0`→indexed) and produce `ts_chunks`. Must FAIL before the periodic reconcile exists — today nothing re-walks the FS after startup. A slower real-watch/real-timer fidelity test belongs in `tests/`.

## Anchors
- Code: `crates/swissarmyhammer-code-context/src/cleanup.rs` (`startup_cleanup`, currently open/promote-only); `swissarmyhammer-tools/src/mcp/tools/code_context/watcher.rs` (`run_watcher`/`process_ok_events` — per-event path); `.../mod.rs` (`index_discovered_files_with_embedder`, `WHERE ts_indexed = 0`).
- Live evidence: `calcutron/.sah/mcp.9852.log` (grep `marking dirty` / `no dirty files to index`); lock `…/T/code-context-ts-bc3927cb1cd02e5607b40c650bbd4c3b.lock`; index `calcutron/.code-context/index.db`.
- Related: ^hdcwqk6 (watcher UPSERT, DONE — event fast-path); leader epic 01KV86XGD5ZFS1F6VQ47A5H2BJ (leader-only LSP spawn), 01KVDEGQ75R48YNFE76X6M3JPZ (no-orphan kill-on-exit). This task is the missing periodic-reconcile / self-heal piece.

_Footnote — the manual recovery this fix eliminates: today the only way out is a hand-run `{op: rebuild index}` or restarting the server so a fresh leader re-runs `startup_cleanup` (the doctor doc says exactly this). The point of this task is that no human should have to do that._ #bug-code-context-indexer-lsp-live-leader