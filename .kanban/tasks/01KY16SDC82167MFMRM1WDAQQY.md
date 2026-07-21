---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01ky1718styf0zyf4qec6xb80y
  text: 'Prioritized to the front of the batch by user direction ("fix this notification and related tasks so we can finally unstick reviews"). This card IS the unsticker: field evidence in ../mlx-swift-lm/.sah/mcp.77297.log shows the bridge armed (token=4) but zero notifications/progress in 1801s → client abort, because all events today are emitted post-scope (fleet stage) and the scope stage alone can outlast the client timeout. Delivery proof must be CLIENT-side over a real byte-stream transport — channel/buffer assertions are explicitly inadmissible per user.'
  timestamp: 2026-07-21T02:10:47.738842+00:00
- actor: claude-code
  id: 01ky174nyf4yy8870vvwfbw87v
  text: 'Definition of done, binding (orchestrator): beyond the automated gates, this card does NOT close until a live field check passes — rebuild the binary (just sah), then run a real `review sha` against this repo through a real stdio MCP client with a progressToken, and observe client-side received notifications: first one within seconds of call start (scope phase), none-to-none gaps never exceeding the keep-alive interval for the whole run duration. Server logs are diagnostics only. The parked mlx run (review sha 4877962..41b8e1f) is the external confirmation after that.'
  timestamp: 2026-07-21T02:12:39.503316+00:00
- actor: claude-code
  id: 01ky17g14s7387b7dfq5qr9pyn
  text: |-
    Picked up. Research done; plan confirmed against the current tree:
    1) fleet.rs gains ReviewProgressEvent::FileScoped{file}; emit_progress goes pub(crate); scope_review gains progress: Option<&ReviewProgressSender> (12 in-crate test call sites updated) and emits one FileScoped per resolved file right after resolve_scope_files (before the whole-set semantic diff + probe pass); synthesize.rs run_review threads its existing progress into scope_review.
    2) review_op.rs: extract the bridge mapping loop into run_review_progress_mapping(event_rx, param_tx, token, keep_alive) with a tokio::select! sleep re-armed per event; REVIEW_PROGRESS_KEEP_ALIVE_INTERVAL=10s; on a stall it re-sends the latest param (identical progress/total). Paused-time unit tests in review_op.rs tests mod.
    3) progress.rs spawn_drain_task: INFO per outgoing send (token, progress, total, full untruncated message) + WARN with full error on send failure. Shared with code_context rebuild index; behavior unchanged.
    4) e2e transport decision: child-process sah is NOT feasible without new CLI config surface for a scripted review agent (set_review_factories is in-process only), so per the card's sanctioned fallback the e2e uses the real McpServer served via rmcp::serve_server over tokio::io::duplex (full JSON-RPC serialize→bytes→deserialize) with a real rmcp client whose overridden on_progress collects receipts. Determinism gate: new PromptGate knob on ScriptedAgentConfig (validators test_support) blocks the agent's first (prime) prompt until the test releases it — while gated, the client must receive scope-phase notifications ("Scoping <file>"), then keep-alive re-sends during a >10s injected stall; both fail on today's code (RED), pass after 1+2. No assertion touches progress_sink or any server-side channel.
    Pool idle timeout is 300s so a ~15s gated stall is safe. TDD order: PromptGate+e2e first (watch fail), then scope unit test (fail→pass), then paused-time keep-alive tests (fail→pass), then full gates.
  timestamp: 2026-07-21T02:18:51.417294+00:00
- actor: claude-code
  id: 01ky18r5qfhxa5xvxpjsz08y1y
  text: |-
    Implementation complete, all gates green. What landed per part:

    1) Scope-phase events: ReviewProgressEvent::FileScoped{file} (fleet.rs); emit_progress is pub(crate); scope_review gained progress: Option<&ReviewProgressSender> and emits one FileScoped per resolved file immediately after resolve_scope_files — before the whole-set semantic diff + probe pass — so the run's first events exist within milliseconds. run_review (synthesize.rs) threads its existing sender in. Unit test scope_review_emits_one_file_scoped_event_per_resolved_file (watched RED: emitted [] vs expected 2 files, then GREEN). validators crate stays rmcp-free (plain enum through the existing tokio mpsc sender).

    2) Keep-alive tick: mapping loop extracted into run_review_progress_mapping(event_rx, param_tx, token, keep_alive) with tokio::select! — the sleep re-arms on every event, disarmed until the first param exists, re-sends the LATEST param verbatim (identical progress/total/message → monotonic by construction). REVIEW_PROGRESS_KEEP_ALIVE_INTERVAL = 10s. Four paused-time unit tests (<0.1s each): re-send during silence + repeats, disarmed before first event, window resets per event, stops when engine sender drops. FileScoped maps to "Scoping <file>" with no counter movement (own unit test).

    3) Send logging: spawn_drain_task (progress.rs, shared with code_context rebuild index) now logs one INFO per outgoing notification with token, progress, total, and the FULL untruncated message, and WARN (was debug) with the full error on send failure. rebuild_index_progress_notifications_test stays green.

    4) Client-side receipt e2e: tests/review_progress_stdio_test.rs. Transport = real McpServer served via rmcp::serve_server over tokio::io::duplex + a real rmcp client (overridden on_progress) — the sanctioned fallback; the child-process sah binary cannot run a scripted review agent (set_review_factories is in-process only) and a real-model run is forbidden. Determinism via new PromptGate knob on ScriptedAgentConfig (validators test_support): the agent's first (prime) prompt latches "entered" and blocks until released. Asserted client-side: scope-phase "Scoping <file>" notifications received while the agent is still blocked on its unanswered first prompt; keep-alive re-sends (identical progress/total) received during a >10s injected stall; token uniformity; total>=progress everywhere; dense completion counter 1..=N (order-free) and a closing progress==total notification. Watched RED first: exactly the production failure — only "Planned 56" received while gated, zero scope events, then (after part 1) a monotonicity trip that exposed a real harness fact.

    Discovery worth remembering: rmcp 1.7 dispatches each incoming notification on its own spawned task (service.rs spawn_service_task), so client on_progress ARRIVAL order is not wire order under bursts — adjacent-pair monotonicity assertions at an rmcp client assert task scheduling, not server behavior. The e2e therefore proves monotonic emission through order-free corollaries (dense 1..=N completion counter with keep-alive exact-duplicates collapsed, total>=progress per message, closing progress==total). The existing HTTP e2e has the same latent race but a 10x smaller notification volume; left untouched and green.

    Also fixed en route: the e2e initially deadlocked because serve_server(...).await only resolves after the client's initialize — server and client must be brought up concurrently over a duplex.

    Gates: cargo nextest -p swissarmyhammer-validators review = 142/142; -p swissarmyhammer-tools -E 'test(review)' = 44/44 (HTTP progress e2e + new stdio e2e included); --test rebuild_index_progress_notifications_test = 1/1; cargo fmt --check clean; cargo clippy -p validators -p tools --all-targets -D warnings clean; cargo check --workspace --all-targets green. New unit tests all <10s; e2e ~21s, no network, no real model.
  timestamp: 2026-07-21T02:40:46.831766+00:00
- actor: claude-code
  id: 01ky19797xtwrtfcsssrw9ktgq
  text: |-
    Adversarial double-check verdict: REVISE with two findings; both fixed exactly as the critic suggested, then all gates re-run green (no second double-check spawn — the fixes implement the critic's own remedies verbatim and were verified by re-running the exact tests, so a re-review of the same tree would add nothing):
    1. e2e step 7 flake hazard: the stall keep-alive assertion compared against the ARRIVAL-last gated param, contradicting the test's own arrival-order caveat. Now order-free: keep-alives must carry the gated phase's max progress / max total (the wire-latest, since counters only grow within the phase).
    2. TEST_KEEP_ALIVE claimed to be "distinct from the production constant" but was also 10s. Now 7s, so the paused-time tests genuinely pin the schedule's shape rather than the production value.
    Final gate results: validators review 142/142; tools -E 'test(review)' 44/44 (HTTP e2e + stdio e2e); rebuild_index_progress_notifications_test 1/1; clippy -D warnings clean (both crates); cargo check --workspace --all-targets green; cargo fmt --check clean. Leaving in doing for /review.
  timestamp: 2026-07-21T02:49:01.949475+00:00
- actor: claude-code
  id: 01ky1b65qdrn3j7cy82t4x3dzn
  text: 'BINDING FIELD CHECK: PASS. Fresh binary (just sah), real `sah serve` child process, real JSON-RPC stdio client (/tmp/sah_progress_probe.py), real review sha HEAD~1..HEAD with progressToken. Client-received results: first notification +1.61s (scope phase, "Scoping ..."), 394 notifications total, max gap 10.02s across the entire 1632.65s (27 min) run incl. start→first and last→result, final result isError=false, probe exit 0. Keep-alive visibly firing at 10.00–10.02s cadence through silent engine stretches (e.g. +1439.69→+1589.73 identical re-sends). Full 405-line transcript preserved in session tool-results. Bonus: the run''s own review output surfaced a real path-traversal finding in resolve_file (scope.rs) — filing as a separate card. Proceeding to /review gate.'
  timestamp: 2026-07-21T03:23:22.733987+00:00
- actor: claude-code
  id: 01ky1ckgf4n78hr0j0ystgkdrj
  text: 'Picked up review finding (2026-07-20 22:23): encapsulate WorkList public fields (change_purpose, validators) behind getters; sweeping scope.rs for sibling public-field structs (ValidatorWork, FileWork, ResolvedScope) per root-cause instruction.'
  timestamp: 2026-07-21T03:48:08.292112+00:00
- actor: claude-code
  id: 01ky1d9tfn95fa9c7yy7mfxc47
  text: |-
    Rework of review finding (WorkList public fields) done at root-cause breadth per orchestrator direction — encapsulated EVERY public-field struct in scope.rs in one pass, not just the cited one:

    - WorkList, ValidatorWork, FileWork: all fields private; each gained a public `new(...)` constructor plus reference/slice getters (`change_purpose() -> &str`, `validators() -> &[ValidatorWork]`, `validator_name()`, `rules()`, `probes()`, `files()`, `path()`, `semantic_diff()`, `changed_symbols()`, `source_slice()`, `probe_results()`). No `into_`-style accessors added: grep + compiler confirmed every consumer (fleet.rs render/plan/collect, synthesize.rs batching + build_candidates, verify.rs) reads by reference — nothing consumes the fields by value, so per rule-of-three no speculative ownership API.
    - ScopeSpec: fields deliberately kept public with a justification doc comment tied to its construction contract — it is a forgiving-input builder surface where the exactly-one invariant is enforced by `resolve()` at resolution time, so private fields would protect no invariant.
    - Call sites updated: fleet.rs (~20 accessor conversions incl. render_run_prime/render_fleet_prompt/render_validator_suffix/render_file_payload — output bytes unchanged, only accessor plumbing), synthesize.rs (build_candidates, logging, test fixtures), fleet/tests.rs (19 WorkList literals -> WorkList::new via scripted transform + fixtures -> constructors; the one field-mutation site became a `file_work_with_slice` fixture variant). swissarmyhammer-tools had zero usages (grep matches were `.validators` dirs and ReviewRequest's own field).
    - Note: a hook-injected round-trip test in scope.rs presumed a positional `ScopeSpec::new(true, None, None, None)`; changed those two lines to the documented struct-literal + Default form instead of adding a 4-positional-arg constructor that contradicts the forgiving-input contract.

    Gates (fresh): cargo nextest -p swissarmyhammer-validators 308/308; -p swissarmyhammer-tools 1427/1427; cargo fmt --check clean; cargo clippy -p validators -p tools --all-targets -D warnings clean; cargo check --workspace --all-targets green. Review finding flipped to [x]; task left in doing. Adversarial double-check verdict to follow.
  timestamp: 2026-07-21T04:00:19.445152+00:00
- actor: claude-code
  id: 01ky1dcgyy5byy7ptf8m8jprxk
  text: 'Review finding fixed at root. WorkList fields change_purpose/validators are now private with getters change_purpose() -> &str and validators() -> &[ValidatorWork]. Root-cause sweep: ValidatorWork and FileWork (the other public structs in scope.rs) encapsulated identically (private fields, new(...) constructors, &str/slice getters); ScopeSpec deliberately keeps public fields as a forgiving-input DTO (invariant enforced by resolve(), rationale documented on the struct). All cross-module call sites migrated: fleet.rs (plan_fan_out, submit/collect_fan_out, prime_run_prefix, render_run_prime, render helpers), synthesize.rs (run_review logging, build_candidates + test helpers), fleet/tests.rs (helpers + all WorkList literals). New round-trip unit test work_list_getters_and_constructors_round_trip_the_private_fields in scope.rs covers every new constructor/getter. Note: a concurrent worker on this card landed part of the same migration; the combined diff was verified whole. Gates: cargo nextest -p swissarmyhammer-validators -p swissarmyhammer-tools 1735/1735 passed; cargo fmt clean; clippy both crates --all-targets -D warnings clean; cargo check --workspace --all-targets clean. Task left green in doing for /review.'
  timestamp: 2026-07-21T04:01:47.998955+00:00
- actor: claude-code
  id: 01ky1de9f79c125q0gmkkg0khr
  text: |-
    Adversarial double-check verdict: PASS. It independently confirmed: (1) diff touches only the 4 expected files; (2) every fleet.rs/synthesize.rs production hunk is a pure accessor substitution (field -> field(), String.clone() -> &str.to_string(), Vec.clone() -> &[T].to_vec()) — value-identical, with trim/iteration-order/header constants untouched and #[derive(Serialize)] output unchanged (field names unchanged), verified by the 20 render/prime/build_candidates tests incl. the byte-stability assertion; (3) zero WorkList/ValidatorWork/FileWork struct literals or field accesses remain outside scope.rs (the .probe_results/.source_slice hits in verify.rs/synthesize.rs are the separate Candidate struct); (4) the 19 fleet/tests.rs conversions + file_work_with_slice are faithful; (5) swissarmyhammer-tools has zero usages.

    Rustdoc note (agent flagged, I verified): grepped `RUSTDOCFLAGS="-D warnings" cargo doc -p swissarmyhammer-validators --no-deps --document-private-items` — ZERO warnings reference any changed link (source_slice/change_purpose/semantic_diff/validators/validator_name/probe_results/changed_symbols); my method-form intra-doc links all resolve. The crate's rustdoc baseline is already dirty (pre-existing private-item links auto_purpose/build_candidates/PRIME_HANDOFF etc. on untouched lines), and rustdoc is not one of this project's gates. No regression introduced.

    Task is green and left in doing for /review.
  timestamp: 2026-07-21T04:02:45.863088+00:00
position_column: doing
position_ordinal: '8280'
title: 'Review keep-alive: emit progress across ALL phases (scope/probes included), log every send, prove delivery over stdio'
---
## What

Field failure (mlx-swift-lm, `.sah/mcp.77297.log`, 2026-07-21): a `review sha` call armed the progress bridge (`00:15:35 DEBUG review: wiring progress bridge to MCP peer token=ProgressToken(Number(4))`) yet the client (Claude Code) aborted at 1801s of silence, and the log contains **zero** `notifications/progress` occurrences. The goal of the whole notification effort is keep-alive — Claude Code resets its MCP tool timeout on progress notifications, so a long review survives **only if notifications are actually received by the client, continuously, and we can prove receipt**. Testing that events land in some channel or buffer proves nothing — the only admissible evidence is a real MCP client's `on_progress` handler firing across a real transport boundary.

Root cause (verified in code): every `ReviewProgressEvent` today is emitted from the fleet stage — `Planned` fires in `run_fleet` (`crates/swissarmyhammer-validators/src/review/fleet.rs:327`) which runs **after** `scope_review` (`crates/swissarmyhammer-validators/src/review/scope.rs:209`, called from `run_review` at `crates/swissarmyhammer-validators/src/review/synthesize.rs:351`). On a large sha range the scope stage (semantic diffs + embedding probes over every changed file on the local model) can alone exceed the client's 30-min timeout — the run dies before the first event exists. Two aggravators: the send path has zero logging (absence of `notifications/progress` in the log is not currently meaningful), and the only e2e (`crates/swissarmyhammer-tools/tests/review_progress_notifications_test.rs`) exercises the in-process HTTP transport, not the stdio transport production uses.

Fix — four parts, one concern (continuous, provable keep-alive):

1. **Scope-phase events**: add early `ReviewProgressEvent` variants (e.g. `FileScoped { file }` emitted per file as the scope stage diffs/probes it, next to the existing variants in `fleet.rs`); thread the existing `Option<&ReviewProgressSender>` from `run_review` (synthesize.rs) into `scope_review` and emit per file. The first event must exist within seconds of the call starting.
2. **Bridge keep-alive tick**: in `spawn_review_progress_bridge` (`crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs`), while the run is live, if no event has been forwarded for ~10s, re-send the latest `ProgressNotificationParam` (identical progress/total — monotonicity preserved). This covers every remaining silent stretch (the run-prime turn, a single long agent turn, verify stage) without per-token spam.
3. **Send-path observability**: in `spawn_drain_task` (`crates/swissarmyhammer-tools/src/mcp/progress.rs`, shared with code_context), log every outgoing notification at INFO (token, progress, total, and the full message — never truncated) and WARN with the full error on a failed `send_notification`. A silent log must now mean "nothing was sent", provably. Logging is diagnostics only — it is NOT delivery evidence.
4. **Prove RECEIPT over stdio, client-side**: new e2e `crates/swissarmyhammer-tools/tests/review_progress_stdio_test.rs` that spawns the real server over the actual stdio transport and connects a real rmcp client whose overridden `on_progress` collects what the CLIENT receives. Strongest form: spawn the actual `sah` binary (`serve` over stdio) as a child process — a true process + pipe boundary (use Cargo's built-binary path via an integration-test harness in the binary's own package `apps/swissarmyhammer-cli`, or an artifact-dep; put the test where the binary is buildable). If a child-process harness is genuinely impractical for a scripted review agent, the minimum acceptable fallback is rmcp client+server connected over a raw duplex byte stream (full JSON-RPC serialize → bytes → deserialize path) — NEVER an in-process channel capture, NEVER `progress_sink`, NEVER asserting on the mapping task's output. All delivery assertions read the client's collected notifications.

Context (separate defect, NOT this card): the same log shows 220× `Notification collector lagged` (claude-agent/src/lib.rs:333, ~1k skipped each) — that is the fleet reply-collection broadcast (NOTIFY_BUFFER=256, drive.rs) lagging under ~216k per-token session updates, a reply-integrity risk to be filed on its own.

Subtasks:
- [x] `FileScoped`-class event variant(s) + sender threaded into `scope_review`, emitted per file during diff/probe
- [x] Keep-alive re-send tick (~10s, tokio paused-time unit-tested) in the review bridge
- [x] INFO per outgoing send + WARN on send failure in `spawn_drain_task` (full payloads; diagnostics only)
- [x] stdio e2e proving CLIENT-SIDE receipt across a real transport boundary (child-process `sah serve` preferred; raw duplex byte stream minimum) — first notification received before the scripted agent's first prompt; keep-alive tick received during an injected stall
- [x] Keep HTTP e2e + all existing review tests green

## Acceptance Criteria
- [x] With a `progressToken`, the first `notifications/progress` is RECEIVED by the e2e client (its `on_progress` fires) before the scripted review agent receives its first prompt — asserted on the client's collected notifications, across a real byte-stream transport
- [x] With events stalled >10s mid-run, the client receives a keep-alive re-send of the latest param (paused-time unit test for the bridge logic + client-side observation in the e2e stall window); wire `progress` never regresses
- [x] Every outgoing progress notification produces one INFO log line carrying token, progress, total, and the full untruncated message; a failed send produces a WARN with the full error (diagnostics — not counted as delivery evidence)
- [x] No delivery assertion anywhere in the new tests inspects a server-side channel, sink, buffer, or log — client-received notifications only; HTTP e2e still passes; no token → zero notifications and identical `ReviewReport`
- [x] `swissarmyhammer-validators` still has no rmcp/MCP dependency (scope events are plain enum values through the existing sender)

## Tests
- [x] Unit test in `crates/swissarmyhammer-validators/src/review/scope.rs` test mod: `scope_review` with a wired sender over a multi-file working scope emits one `FileScoped` per file before returning (engine-side event-existence test — allowed here because it tests emission, not delivery)
- [x] Paused-time unit test in `crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs`: no engine events for >10s → keep-alive re-send of the last param
- [x] New e2e `crates/swissarmyhammer-tools/tests/review_progress_stdio_test.rs` (or in `apps/swissarmyhammer-cli/tests/` if the child-process harness lives with the binary): real client over real stdio transport, scripted review agent; asserts on the CLIENT's received notifications: ≥1 before the first agent prompt, keep-alive during an injected stall, token echoed, monotonic progress
- [x] Run: `cargo nextest run -p swissarmyhammer-validators review` and `cargo nextest run -p swissarmyhammer-tools -E 'test(review_progress)'` (plus the CLI-package test if the harness lands there) — green, scripted agents only, unit tests <10s

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #review #mcp #progress

## Review Findings (2026-07-20 22:23)

- [x] `crates/swissarmyhammer-validators/src/review/scope.rs:82` — Public struct fields on WorkList violate the future-proofing rule; two public fields (change_purpose, validators) lock the struct's internal representation. Make both fields private and provide getter methods: `pub fn change_purpose(&self) -> &str` and `pub fn validators(&self) -> &[ValidatorWork]`, allowing the internal representation to evolve without breaking downstream code.

### Root-cause sweep (2026-07-20)

- [x] Sibling public-field structs in scope.rs — `ValidatorWork` and `FileWork` — encapsulated the same way (private fields + `new(...)` constructors + `&str`/slice getters); all cross-module call sites in fleet.rs, fleet/tests.rs, and synthesize.rs migrated from field access to getters/constructors, so the next review round finds zero recurrences of this rule in scope.rs.
- [x] Judgment call: `ScopeSpec` deliberately keeps public fields — it is a forgiving-input DTO whose exactly-one-selector invariant is enforced by `resolve()` at resolution time, never at construction; private fields would protect no invariant while costing the struct-literal (`..Default::default()`) ergonomics the type exists for. Rationale documented on the struct itself; nothing outside the crate constructs it today.