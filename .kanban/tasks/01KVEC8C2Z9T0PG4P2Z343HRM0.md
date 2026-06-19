---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvf0askv6mn8534sz7v2pcv4
  text: 'Picked up by /finish $diagnostics, after ^4rjtgsj → done. Last ready task in scope; the advisory filed during ^cxz8vs4. Fix: thread a cooperative cancellation signal into swissarmyhammer_diagnostics::subscribe_diagnostics_over_bus (Arc<AtomicBool> or tokio_util::CancellationToken — reuse whatever the codebase already uses, no new dep if avoidable) checked each loop iteration; spawn_blocking tasks can''t be force-aborted mid-recv, so cooperative check (loop already wakes every ≤500ms on recv_timeout). spawn_follower_diagnostics_subscriber holds the token; handle_promotion_result/spawn_reelection_loop signals cancellation on promotion BEFORE the cold re-spawn starts the leader-side publish path. Acceptance: subscriber stops within ~1 loop interval of promotion (no lingering blocking task, no self-logging of new leader''s own diagnostics); unit/integration coverage that the loop exits when the flag is set. Driving implement → test → review.'
  timestamp: 2026-06-19T03:55:56.411441+00:00
- actor: claude-code
  id: 01kvf0jhgzfvnfgn1sh7w2pt9m
  text: 'Implemented. Primitive: Arc<AtomicBool> checked with Ordering::Relaxed — confirmed it''s the existing in-tree cancellation primitive (file_watcher.rs uses Arc<AtomicBool>; tokio_util::CancellationToken is used in llama-agent but NEITHER swissarmyhammer-diagnostics NOR swissarmyhammer-tools depends on tokio-util, so per the card I did not add a new dep). subscribe_diagnostics_over_bus(backend, cancel: &Arc<AtomicBool>, on_update) now checks cancel.load(Relaxed) at the top of each loop iteration (loop already wakes ≤500ms on recv_timeout). Where promotion signals cancel: spawn_follower_diagnostics_subscriber returns Option<Arc<AtomicBool>>; it''s threaded through spawn_reelection_loop into handle_promotion_result, which on the Outcome(Ok(Some(shared_db))) promotion arm calls cancel.store(true) BEFORE start_indexing_workers_after_promotion (the leader-side publish path). Leader startup path passes None. Test (TDD): subscribe_helper_loop_exits_when_cancel_flag_is_set in tests/diagnostics_bus_pubsub.rs — real in-process proxy/leader (mirrors existing harness), proxy kept ALIVE (so only the flag can stop the loop, not a disconnect), set flag, poll thread.is_finished() and join within 2s. Verified red-green-red: disabling the cancel check makes the test fail "loop did not exit within 2s". Running full verification next.'
  timestamp: 2026-06-19T04:00:10.271145+00:00
- actor: claude-code
  id: 01kvf17yevjz2zvpcvzbtjn6hn
  text: 'Verification green; moved to review. cargo test -p swissarmyhammer-diagnostics: lib 62 + diagnostics_bus_pubsub 3 (incl. new subscribe_helper_loop_exits_when_cancel_flag_is_set) + leader_follower_request_ipc 2 + leader_watcher 2, 0 failed. cargo test -p swissarmyhammer-tools: lib 1093 + 163/17/8/2/7/8/1*… across suites, 0 failed. clippy -p swissarmyhammer-diagnostics and -p swissarmyhammer-tools --all-targets -D warnings: 0 warnings each. cargo fmt --check: clean. really-done adversarial double-check: PASS (verified cancel signaled before leader publish path, leader path passes None, Arc<AtomicBool>/Relaxed matches in-tree patterns file_watch_stopped/lsp_worker::ShutdownFlag/proxy::stop, test holds proxy alive so only the flag ends the loop, all 4 changed signatures'' callers updated, no race). .config/nextest.toml left untouched. Not committed (orchestrator handles after review).'
  timestamp: 2026-06-19T04:11:51.643284+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffc980
project: diagnostics
title: Abort follower diagnostics-bus subscriber on promotion
---
## What
`spawn_follower_diagnostics_subscriber` (in `crates/swissarmyhammer-tools/src/mcp/server.rs`) starts a `tokio::task::spawn_blocking` loop running `swissarmyhammer_diagnostics::subscribe_diagnostics_over_bus`. It runs for the process lifetime and is NOT stopped when the follower is promoted to leader.

Because the bus backend address is deterministic by workspace hash, after a follower→leader promotion the orphaned subscriber silently reconnects to the (now own) proxy and debug-logs the leader's own diagnostics. A ZMQ ipc disconnect surfaces as `EAGAIN`, not the `Disconnected` error string the loop breaks on, so the loop only ends at process/context teardown.

## Impact (low)
One lingering blocking task per promoted-follower + benign self-logging at debug. No wrong output, no deadlock, no hot spin (the loop blocks on a 500ms `recv_timeout`). Surfaced as an advisory by double-check on ^cxz8vs4 (cross-process fan-out + leader watcher); intentionally deferred there to avoid scope creep.

## Fix
Thread a cancellation signal into `subscribe_diagnostics_over_bus` (e.g. an `Arc<AtomicBool>` / `tokio_util::CancellationToken` checked each loop iteration) so the follower subscriber can be stopped. `spawn_blocking` tasks cannot be force-aborted mid-`recv`, so the loop must cooperatively check the flag (it already wakes every ≤500ms on `recv_timeout`). Have the re-election loop (`handle_promotion_result` / `spawn_reelection_loop` in server.rs) signal cancellation when the workspace is promoted, before the cold re-spawn starts the leader-side publish path.

## Acceptance Criteria
- [ ] A follower-started diagnostics-bus subscriber stops within ~1 loop interval of promotion (no lingering blocking task, no self-logging of the new leader's own diagnostics).
- [ ] Unit/integration coverage for the cancellation (the subscriber loop exits when the flag is set).

#diagnostics

## Review Findings (2026-06-18 23:13)

Verdict: clean — 0 blockers. The three engine findings are confirmed by direct inspection but are cosmetic style nits, not actionable blockers; recorded for history. Load-bearing checks all pass:

- Cancel placement: `cancel.store(true)` fires ONLY on the genuine follower→leader arm (`PromotionState::Outcome(Ok(Some(shared_db)))`), and BEFORE the bus-frontend read and `start_indexing_workers_after_promotion` (leader publish path). `AlreadyLeader` / `Ok(None)` / `Err` arms do not signal. Leader-startup path passes `None`; `if let Some` guards against panic. The returned `Arc` is the same allocation cloned into the spawn_blocking loop (`loop_cancel` = `Arc::clone(&cancel)`, original returned).
- Cooperative cancel: flag checked at the top of each loop iteration before `recv_timeout`; `None` timeout `continue`s back to the check; exit within one ≤500ms interval. `Ordering::Relaxed` is correct (single flag, no other memory-ordering dependency, no missed-wakeup hazard since the loop re-polls).
- Primitive: `Arc<AtomicBool>` is the existing in-tree cancellation pattern; neither swissarmyhammer-diagnostics nor swissarmyhammer-tools depends on `tokio-util`, so `CancellationToken` would have been a new dep — correct no-new-dep, no-duplicate choice.
- Test `subscribe_helper_loop_exits_when_cancel_flag_is_set`: keeps the leader alive (drops it only after join), so the `"disconnected"` arm cannot be the stop cause — only the flag can. Detects exit deterministically via `is_finished()` poll + bounded 2s deadline, not a fixed sleep. Model-free, <2s. Truly isolates the flag.
- No regression: fan-out path untouched; all changed-signature call sites updated.

### Nits (non-blocking, recorded for history)
- [ ] `crates/swissarmyhammer-diagnostics/src/bus.rs` — the `if e.to_string().contains("disconnected")` disconnect-check nests 4 levels (loop → match → arm → if). Could flatten via an early-exit `let should_break = ...` after the match. Stylistic; the inline comment already explains why the check exists.
- [ ] `crates/swissarmyhammer-diagnostics/tests/diagnostics_bus_pubsub.rs` — the cancel-exit join deadline uses `Duration::from_millis(2000)` while `MESSAGE_RECEIVE_TIMEOUT_MS = 2000` exists. Semantically distinct (join deadline vs message-receive timeout) but shares the literal; consider a named `CANCEL_EXIT_DEADLINE_MS` to avoid drift.
- [ ] `crates/swissarmyhammer-diagnostics/tests/diagnostics_bus_pubsub.rs` — the 20ms `is_finished()` poll interval is an unnamed literal; consider a named `POLL_CHECK_INTERVAL_MS` for consistency with the other timeout constants.