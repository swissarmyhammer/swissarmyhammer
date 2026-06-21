---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvd7vdek59sn2k841zqxy7w7
  text: |-
    Picked up. Deps satisfied: swissarmyhammer-lsp fan-out (session.subscribe() -> broadcast::Receiver<DiagnosticUpdate> + diagnostics_for(uri) cache, commit 35680a) and the diagnostics record/config types (commit 29bd5bb) both exist.

    Plan: new module settle.rs in swissarmyhammer-diagnostics with:
    - `Timer` trait (fn sleep(dur) -> Pin<Box<dyn Future + Send>>) + `TokioTimer` prod impl; tests use a manual virtual-clock fake (oneshot-backed) for deterministic, model-free timing.
    - `SettleOutcome { Settled(Vec<DiagnosticRecord>) | Pending }`.
    - core `settle_stream(rx, uris, config, timer, initial)`: tokio::select! biased over [hard timeout, rx.recv(), debounce]; each watched-uri update replaces per-uri latest state and resets the debounce sleep; quiescence -> Settled(latest), hard timeout -> Pending, channel Closed -> Settled(what we have).
    - session wrapper `settle(session, uris, config, timer)`: subscribe + seed initial from cache, then settle_stream.
    - Records honor config severities + per_report_cap; containing_symbol left None (enrichment is a consumer's job).

    Hard timeout: adding `settle_hard_timeout` (default 5s) to DiagnosticsConfig as the backstop. Leaving settle_window at the existing 300ms default — the prior task pinned it "short for responsiveness" with a test, so I won't contradict that; the 5s hard timeout is the "few seconds" generous backstop the design note refers to.
  timestamp: 2026-06-18T11:28:52.179911+00:00
- actor: claude-code
  id: 01kvd8grzc551031ntzhfkc906
  text: |-
    Implementation landed; moving to review.

    Files:
    - crates/swissarmyhammer-diagnostics/src/settle.rs (new): Timer trait + TokioTimer; SettleOutcome { Settled | Pending }; settle_stream core (biased tokio::select! over [hard timeout, rx.recv(), debounce]; per-watched-uri latest state; debounce reset per update; quiescence -> Settled, hard timeout -> Pending, Closed -> Settled, Lagged -> resync-from-cache then reset); settle() session wrapper (subscribe-then-seed, wires resync to session cache).
    - config.rs: added settle_hard_timeout (DEFAULT 5s) + test.
    - lib.rs: module + re-exports. Cargo.toml: tokio dep.

    Verification (all fresh, green): `cargo test -p swissarmyhammer-diagnostics` = 32 passed / 0 failed; `cargo clippy --all-targets -- -D warnings` clean; `cargo fmt --check` clean. Tests are model-free with a virtual-clock fake (<0.01s).

    really-done + adversarial double-check run. Double-check verdict: core acceptance criteria all hold (only-final-set, hard-timeout-Pending, injectable timer verified; tests are real, not phantom; could not induce flakiness in 30 runs). It returned REVISE with 3 findings, ALL now fixed:
    1. Lag mid-settle could emit a stale set across multiple uris (broadcast evicts oldest; a watched uri's update can be dropped under churn from other docs while its latest still lives in the per-uri cache) — now resyncs from cache on RecvError::Lagged; new test `lag_recovers_latest_from_resync_instead_of_emitting_stale_set` (capacity-1 ring + cross-uri eviction) proves it (pre-fix it would settle empty).
    2. Session-facing settle() (the real subscribe-then-seed path) was untested — added `settle_session_seeds_from_cache_and_settles` and `settle_session_captures_update_published_after_subscribe` using a NullTransport LspSession driven via handle_publish_diagnostics, no live server.
    3. Test determinism implicitly depended on the current-thread scheduler — pinned `#[tokio::test(flavor = "current_thread")]` on every async test and documented the requirement on spawn_settle.

    Design note: left settle_window at the existing 300ms default (prior task pinned it short with a test); the new 5s hard timeout is the generous backstop. settle_stream stays a pure core (no session handle) with resync injected as a closure, so it remains driven entirely by the fan-out channel + injectable clock.
  timestamp: 2026-06-18T11:40:32.108464+00:00
- actor: claude-code
  id: 01kvd9wt75smx0s0c6kjkhqp8j
  text: |-
    Second review pass (scoped `review file` on the 3 changed diagnostics files) returned 1 blocker + 2 warnings — all triaged:

    1. [BLOCKER, fixed] Duplicate uri-filter chain in settle.rs (initial seed + Lagged resync both did the same `.filter(watched.contains).collect()`). Extracted `fn filter_to_watched(map, watched)` and call it at both sites.

    2. [WARNING, fixed] `uri_to_path` in settle.rs duplicated `uri_to_file_path` in swissarmyhammer-code-context/src/ops/lsp_helpers.rs. Resolved per the no-duplicate rule by adding ONE canonical `file_path_from_uri` to the shared `swissarmyhammer-lsp` crate (new `uri` module, both crates already depend on lsp), using it from diagnostics, and re-pointing code-context's `uri_to_file_path` to delegate to it. Now a single implementation; both consumers reuse it.

    3. [WARNING, declined w/ justification] "Make DiagnosticsConfig fields private + getters/builder." Out of scope: the all-public-field design is a pre-existing decision from the crate-creation task (29bd5bb); the whole crate constructs configs via `DiagnosticsConfig { .. ..Default::default() }`, and this task only added one field. Converting to a builder is an unrelated API refactor of existing committed code, not part of the settle engine.

    Verification after fixes (all fresh, green): diagnostics `cargo test` 32 passed/0 failed; lsp `cargo test` 216 passed/0 failed (incl. 3 new `uri` module tests); `cargo clippy --all-targets -D warnings` clean across swissarmyhammer-lsp + swissarmyhammer-diagnostics + swissarmyhammer-code-context; fmt clean.

    Note on process: these were NEW findings the first 15-validator pass did not surface (same code) — the known review-engine non-determinism. I'm fixing the legitimate, in-scope ones and verifying the concrete machine-checkable acceptance criteria directly rather than looping the engine indefinitely (per prior guidance on review-churn).
  timestamp: 2026-06-18T12:04:35.173785+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffbb80
project: diagnostics
title: Settle/debounce engine for diagnostics quiescence
---
## What
Language servers re-flow diagnostics as they analyze; never report mid-analysis state. In `swissarmyhammer-diagnostics`, add a settle engine that subscribes to the session's diagnostics fan-out and waits for a quiescence window before emitting a settled set; on timeout it returns `pending` as a backstop for pathological analysis only.

- `settle(uris, config) -> SettleOutcome { Settled(Vec<DiagnosticRecord>) | Pending }`: subscribe to the per-uri broadcast, reset a debounce timer on each update, emit when no update arrives within `settle_window`, or `Pending` after a hard timeout.
- Generous settle by default (a few seconds in-tool beats an extra model turn — see design "do more per call").
- Pure async logic driven by the fan-out channel; the timer source must be injectable so tests are deterministic and fast.

## Depends on
- "Capture publishDiagnostics and add in-process fan-out in swissarmyhammer-lsp" (the subscribe source)
- "Create swissarmyhammer-diagnostics crate: report types, config, lsp_types mapping" (record/config types)

## Acceptance Criteria
- [x] `settle()` emits only the settled diagnostic set after a quiescence window; never an intermediate re-flow.
- [x] Hard timeout yields `Pending`.
- [x] Timer/clock injectable for deterministic tests.

## Tests
- [x] `cargo test -p swissarmyhammer-diagnostics`: scripted revision stream (e.g. 3 rapid updates then quiet) asserts only the final settled set is emitted; a never-quiescing stream asserts `Pending` at timeout. Model-free, uses a fake clock, <1s.

## Workflow
- Use `/tdd` — write the scripted-revision-stream test first. #diagnostics

## Review Findings (2026-06-18 06:41)

> ⚠️ 1/15 review tasks failed — results are INCOMPLETE.

### Nits
- [x] `crates/swissarmyhammer-diagnostics/src/settle.rs:379` — Hardcoded broadcast channel capacity 256 appears 4 times across independent tests (lines 379, 427, 534, 571) for identical purpose (standard test broadcast setup). Should be a named constant to reduce duplication and clarify intent. Define a test module constant `const TEST_BROADCAST_CAPACITY: usize = 256;` near the test module start and use it across all 4 instances. — FIXED: added `const TEST_BROADCAST_CAPACITY: usize = 256;` to the test module and replaced all seven `(256)` literals; the lag test's intentional `(1)` capacity is left literal (it deliberately forces `RecvError::Lagged`). Tests 32 passed, clippy/fmt clean.