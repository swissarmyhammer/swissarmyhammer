---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvdx4t5yx1jtnkqfr435xdba
  text: 'Picked up by /finish $diagnostics (scoped-batch). Review column empty; this is the first ready todo by ordinal. Dependencies now satisfied: ^7a5h2bj (leader-gated LSP spawn) merged to diagnostic @634fa6175, and ^9fq036d (diagnose() core API) landed @64c768603 — so the leader now owns exactly one LspSession and there is a request API surface to serve. Driving implement → test → review. Per the card: add a real request/response listener (prefer tokio UnixListener at LeaderGuard::socket_path(), which today is only a marker file, never bound) carrying the SAH request API (diagnose + code-context queries, NOT raw LSP), leader multiplexes concurrent follower calls onto the single stdio session and demuxes by envelope id, followers become socket clients with no local LSP spawn + typed leader-pid error on connect failure. Build generic enough for rebuild-index follower writes to ride later.'
  timestamp: 2026-06-18T17:41:00.222850+00:00
- actor: claude-code
  id: 01kvdx8yfssp1ct1kdxsc9qf1e
  text: |-
    Research done. Verified: LeaderGuard::socket_path()/FollowerGuard::socket_path() return a .sock path that NOTHING binds — it is only a marker (fs::write in tests) removed on drop; the ZMQ XPUB/XSUB bus in proxy.rs/bus.rs binds SEPARATE ipc addresses (discovery::ipc_addresses) for pub/sub, not req/resp. peek_leader_pid(lock_path) reads the leader PID from the flock file. diagnose() lives in swissarmyhammer-diagnostics::diagnose, signature diagnose<C,T,D>(session: &LspSession<C>, paths, config, dependents, timer) -> DiagnosticsReport. LspSession is the single owned session (request/notify/with_client). server.rs::do_initialize_code_context already gates supervisor spawn on ws.is_leader(); follower branch spawns NOTHING and only polls for promotion.

    Transport decision: tokio::net::UnixListener bound at socket_path() per card preference. Will add a new module `request_ipc` (or similar) in swissarmyhammer-leader-election carrying a generic correlated framed envelope {id, method, params} -> {id, result|error} (newline-delimited JSON length-framed), a server that takes a handler closure (generic so rebuild-index writes can ride later) and a client that round-trips. Typed not-leader error via peek_leader_pid. Following /tdd: unit envelope/correlation test first.
  timestamp: 2026-06-18T17:43:15.705010+00:00
- actor: claude-code
  id: 01kvdxnrtpyk6h459gb95rqqsd
  text: |-
    Implementation landed (TDD, RED→GREEN watched).

    WHAT I BUILT
    - crates/swissarmyhammer-leader-election/src/request_ipc.rs (NEW): generic correlated request/response transport over the election socket.
      - RequestEnvelope {id, method, params} / ResponseEnvelope {id, result|error}, newline-delimited compact-JSON framing; encode/decode helpers.
      - RequestServer::bind(socket_path) (UnixListener; unlinks stale socket; removes on drop) + serve(handler) — accepts connections, spawns a per-request task per line, writes correlated responses through a shared writer mutex so concurrent completions can't interleave bytes.
      - RequestClient::connect(socket_path, lock_path) + call(method, params) — one background reader task demuxes responses to per-call oneshot channels by id, so N concurrent calls over one connection each get their own correlated reply. Connect failure → IpcError::NotLeader { leader_pid: peek_leader_pid(lock_path), source } (typed "leader is PID X"). Remote error → IpcError::Remote. Dropped conn → ConnectionClosed (clears pending so callers never hang).
      - Exported from lib.rs. Crate gained serde + tokio deps.
    - crates/swissarmyhammer-diagnostics/src/request_api.rs (NEW): the SAH request API layer onto the generic transport.
      - dispatch(session, dependents, timer, config, method, params): "diagnose" {paths:[..]} → diagnose() → DiagnosticsReport (JSON); "lsp_request" {method, params} → session.request() (covers get definition/hover/references etc.); unknown method → error string (no panic).
      - serve_session_requests(server, session, dependents, config): leader-side — runs RequestServer::serve routing every follower request through dispatch onto the ONE LspSession. The session's own client mutex serializes stdio, so concurrent handler invocations multiplex safely; the IPC layer demuxes by id. Generic enough that a rebuild-index write op is just another method arm.
      - SessionRequestClient (follower): connect() + diagnose(paths) + lsp_request(method, params). Owns NO session.
      - Crate gained swissarmyhammer-leader-election dep; serde_json moved to a normal dep.

    TRANSPORT DECISION: tokio UnixListener at LeaderGuard::socket_path() (the card's preferred option). Reasons: the socket path already exists per-workspace and was previously only a marker; the ZMQ bus is XPUB/XSUB (no reply path) so it cannot carry request/response; the leader is the sole client of the stdio LSP either way, so a transport-uniform framed envelope is all that's needed. No new ZMQ ROUTER/DEALER discovery address introduced.

    TESTS
    - Unit (model-free, ms): leader-election request_ipc — envelope round-trip, ok/err shapes, interleaved-concurrent-calls correlation (out-of-order replies routed by id), connect-to-absent-leader → NotLeader with pid, remote-error → Remote. diagnostics request_api — dispatch diagnose encodes a report (NullTransport+ManualTimer), unknown method errors, lsp_request on dead session errors, parse_paths rejects non-strings.
    - Integration (rust-analyzer-gated), crates/swissarmyhammer-diagnostics/tests/leader_follower_request_ipc.rs: real rust-analyzer leader session bound at a socket; follower (SessionRequestClient, no local LSP) fires 8 concurrent calls mixing diagnose + textDocument/definition; asserts all 8 correctly correlated. Second test: follower connect to absent leader → typed error rendering the leader PID. PASSED (2.40s, not skipped — ra present).

    SCOPE DECISION (production rewiring deferred — flagging per implement contract): the card's acceptance criteria are about the multiplexer MECHANISM (listener carries the SAH request API; leader multiplexes/demuxes by id; followers satisfy diagnose/query via socket with typed not-leader error; generic for rebuild-index). All of that is built and verified end-to-end with a real server. I did NOT rewire the live MCP path (server.rs leader does not yet bind/serve the socket at startup, and the diagnostics tool's produce_outcome — crates/swissarmyhammer-tools/src/mcp/tools/diagnostics/mod.rs — still returns settled_empty() for followers instead of routing through SessionRequestClient). That rewiring is a contained follow-on: (1) expose socket_path() on CodeContextWorkspace (election already knows it), (2) leader spawns serve_session_requests at startup in do_initialize_code_context's Leader branch, (3) produce_outcome falls back to SessionRequestClient::connect(socket, lock) when no in-process session. I judged that cross-crate live rewiring to be beyond this card's stated criteria and the "no scope creep" guidance; recommend a follow-up card to wire it live. The in-process-subagent path (share parent session Arc directly) already works today via the existing shared session.
  timestamp: 2026-06-18T17:50:15.894554+00:00
- actor: claude-code
  id: 01kvdy15eethqsqdqcqnjnfngx
  text: |-
    really-done / adversarial double-check returned REVISE with one medium + one low defect in request_ipc.rs (finding 3 — blocking session.request inside async task — correctly carried to the deferred wiring card). Fixed both:

    (1) MEDIUM — client-drop liveness/fd leak. The reader task held a strong Arc<ClientInner>, and ClientInner owns the write half, so dropping every RequestClient clone never dropped the write half → server never saw EOF → connection task + reader task + 2 fds leaked per dropped follower (unbounded in a long-lived leader). FIX: reader task now holds a Weak<ClientInner> and upgrade()s per response (breaks the cycle); dropping the last client drops inner → drops the write half → server sees EOF → reader's next_line returns EOF and the loop ends. Regression test added: dropping_the_last_client_closes_the_connection_so_the_peer_sees_eof (raw server stream reads 0 bytes within 2s after client drop).

    (2) LOW — stale pending entry on send-path failure. call() inserted the pending oneshot before encode/write/flush; on a send failure it returned the error but left the dead sender in the map. FIX: extracted write_request(); on any send-path error, remove the pending entry before returning. Regression test added: call_on_a_dead_connection_does_not_leak_a_pending_entry (server closes immediately; call resolves to an error within 2s, never hangs).

    Re-verified (fresh): cargo test -p swissarmyhammer-leader-election -p swissarmyhammer-diagnostics → leader-election 88 passed / 0 failed (was 86, +2 regression tests), diagnostics lib 43 / 0, integration 2 / 0 (rust-analyzer exercised, 2.39s). cargo clippy -p swissarmyhammer-leader-election --all-targets -- -D warnings → 0 warnings. cargo fmt --check → clean.
  timestamp: 2026-06-18T17:56:29.262993+00:00
- actor: claude-code
  id: 01kvdze043naah6x73hn8tnps6
  text: |-
    Addressed all 3 warnings + 7 nits from the 2026-06-18 review checklist. Per-finding:

    WARNINGS
    1. parse_paths path-traversal (request_api.rs): now rejects any entry with a `..` parent-dir component (`Path::components().any(ParentDir)`) — the real traversal escape vector — with a clear error and a doc comment. DEVIATION (justified, logged in the card): the finding's example also said to reject absolute paths, but I did NOT, because `diagnose` is contractually an absolute-space API — produce_outcome in swissarmyhammer-tools/.../diagnostics/mod.rs ("diagnose works in absolute space"; resolve_paths/absolutize) and inline_diagnostics.rs ("already-filtered, absolute, diagnosable paths") both hand it absolute repo paths. Rejecting absolute paths would reject every legitimate follower call and break the gated integration test (which diagnoses an absolute main.rs). New unit tests: parse_paths_rejects_parent_dir_traversal, parse_paths_accepts_absolute_repo_paths.
    2. RequestServer Debug (request_ipc.rs): added `#[derive(Debug)]` (UnixListener + PathBuf are both Debug, so a derive suffices).
    3. Reader-task deep nesting (request_ipc.rs): extracted `deliver_response_line(line, &Weak<ClientInner>) -> ReaderStep` with a small `ReaderStep { Continue, Stop }`. Loop is now flat — empty/undecodable/dropped-client/decode-and-deliver invariants live in the helper via early-return let-else guards; the loop just matches Stop to break. Semantics unchanged (verified by double-check): EOF/error arm still clears pending; Stop path correctly does not (inner already dropped); no lock held across an await.

    NITS (all in request_ipc.rs tests unless noted)
    - HANDLER_BASE_DELAY_MS = 50, DELAY_MULTIPLIER = 5 (delay calc), CONNECT_RETRY_DELAY_MS = 5 (both connect-retry sleeps), TEST_TIMEOUT_SECS = 2 (both regression timeouts) — all magic numbers now named constants.
    - Inlined the single-call `which_on_path()` helper into `rust_analyzer_available()` in the integration test; wrapper removed.

    VERIFICATION (fresh, this session):
    - cargo test -p swissarmyhammer-leader-election -p swissarmyhammer-diagnostics → leader-election 88 passed / 0 failed; diagnostics lib 45 / 0 (+2 parse_paths tests); integration 2 / 0 (rust-analyzer EXERCISED, not skipped, 2.41s).
    - cargo clippy -p swissarmyhammer-leader-election -p swissarmyhammer-diagnostics --all-targets -- -D warnings → 0 warnings (exit 0).
    - cargo fmt --check → clean (exit 0).
    - really-done adversarial double-check → PASS (no correctness/completeness/intent-drift/blast-radius problems; absolute-path deviation judged sound).

    All 10 checkboxes flipped to [x] in the description. Moving back to review. Did NOT touch the refuted blocker (.leader_pid thiserror shorthand) or the deferred live-MCP wiring (^4rjtgsj).
  timestamp: 2026-06-18T18:20:58.371357+00:00
- actor: claude-code
  id: 01kvdzff46v73z17bfcp3qkcff
  text: '/finish disposition → DONE. Review pass 1 (reviewer agent): 0 actionable blockers (the lone engine "blocker" — `match .leader_pid` — refuted & verified to compile; it''s thiserror format-shorthand inside #[error(...)]), all 3 acceptance criteria confirmed met and machine-verified by the rust-analyzer-gated integration test (real leader, follower over a real socket, 8 concurrent correlated diagnose + textDocument/definition calls, typed leader-pid error on absent leader); reuse confirmed (binds existing socket_path(), reuses peek_leader_pid, no second election/duplicate envelope). 3 warnings + 7 nits surfaced → implement pass 2 fixed all 10 (path-traversal `..` rejection in parse_paths [absolute paths intentionally allowed — diagnose is absolute-space], Debug on RequestServer, flattened demux loop via deliver_response_line/ReaderStep, named constants, inlined test helper); really-done adversarial double-check returned PASS. Not re-running `review working` (would only re-sweep the tree for fresh tangential nits per the known churn pattern) — acceptance criteria are machine-verified and prior findings cleared. Deferred live-MCP wiring tracked as follow-on ^4rjtgsj (depends on this). Next: /commit a local rollback point (commit only, not pushed).'
  timestamp: 2026-06-18T18:21:46.502515+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffc080
project: diagnostics
title: Request/response multiplexer over the election socket (follower→leader request API)
---
## What
**New IPC** — the leader-election crate today only has a ZMQ XPUB/XSUB pub/sub bus (`proxy.rs`/`bus.rs`), NOT request/response. The design needs the leader to front a request endpoint with the **SAH request API** (`diagnose(paths)`, references, hover — NOT raw LSP) and multiplex follower calls onto its single stdio session, demultiplexing responses. Followers become socket clients instead of constructing their own session. (This overlaps the IPC explicitly deferred by the `rebuild-index` project — "cross-process IPC for followers to dispatch writes to a running leader" — build it once, here, generically.)

- **Transport (net-new, decided):** `LeaderGuard::socket_path()` returns a `.sock` PATH but **nothing binds or listens on it today** (verified: it is only written as a marker via `fs::write` in tests and removed on drop; the pub/sub bus binds *separate* ZMQ ipc addresses in `proxy.rs`). So this task adds a real request/response listener. Bind a **unix-domain-socket server (`tokio::net::UnixListener`) at `socket_path()`** (or, if staying within ZMQ, a ROUTER/DEALER pair on a new discovery address — pick UnixListener unless there's a reason to reuse zmq). Define a small correlated framed envelope `{id, method, params}` → `{id, result|error}`. Keep it transport-uniform (identical whether the underlying LSP server is stdio or natively socketed) since the leader is the sole client either way.
- Leader side: accept connections on the listener, serve the request API, dispatch onto the one `LspSession`; serialize/multiplex concurrent follower requests onto the single stdio id space and demux responses back by envelope id.
- Follower side: a client that implements the same request API surface (`diagnose`, code-context query ops) by round-tripping to the leader socket; `peek_leader_pid` used for typed "leader is PID X" errors on connect failure.
- **Subagents:** in-process subagents share the parent's session directly; out-of-process subagents connect to the leader socket like any other follower.

## Depends on
- "Leader-per-workdir: only the leader spawns the LSP session" (7a5h2bj)
- "diagnose(paths) core API with capped broken-dependents" (9fq036d) — the request API surface to serve

## Acceptance Criteria
- [ ] A request/response listener (UnixListener at `socket_path()`, or documented ZMQ ROUTER/DEALER) carries the SAH request API (diagnose + code-context queries), not raw LSP.
- [ ] Leader multiplexes concurrent follower requests onto one stdio session and demuxes responses correctly by envelope id.
- [ ] Followers satisfy `diagnose`/query calls via the socket with no local LSP spawn; connect/serve failures return a typed not-leader/leader-pid error.

## Tests
- [ ] Integration: leader + follower in separate processes (or tasks) over a real socket; follower issues N concurrent `diagnose`/`get definition` calls, assert correct correlated responses and that only the leader's single rust-analyzer is running (gated on rust-analyzer).
- [ ] Unit: envelope encode/decode + id correlation under interleaved requests (model-free, <1s).

## Workflow
- Use `/tdd`. Build the multiplexer generic enough that rebuild-index follower writes can ride it later. #diagnostics

## Review Findings (2026-06-18 12:58)

### Blockers
- [x] `crates/swissarmyhammer-leader-election/src/request_ipc.rs:148` — The thiserror error attribute uses `.leader_pid` as a standalone expression, but `.` (field access) requires an object before it. `.leader_pid` without an object is invalid Rust syntax and will not compile. Change `match .leader_pid` to `match leader_pid` to access the field binding that exists in the variant pattern. The thiserror macro will properly scope this in the generated Display impl.
  - **REFUTED by the reviewer (false positive — do not act on this).** The flagged construct is at the `#[error(...)]` attribute on `IpcError::NotLeader`. `.leader_pid` is thiserror's documented format-arg shorthand inside its own `#[error(...)]` attribute (it rewrites `.field` to the variant field), not free-standing Rust. Verified by a fresh build: `cargo build -p swissarmyhammer-leader-election` → exit 0, `Finished`. The crate compiles; there is no compile error. No change required.

### Warnings
- [x] `crates/swissarmyhammer-diagnostics/src/request_api.rs` (`parse_paths`) — Path Traversal: unvalidated file paths from user-supplied JSON are passed to `diagnose()` without validation for `..` or absolute paths. Validate paths before returning from `parse_paths()`: reject paths containing `..` components or that are absolute. Example: `let path = std::path::Path::new(path_str); if path.components().any(|c| matches!(c, std::path::Component::ParentDir)) || path.is_absolute() { return Err("invalid path".into()); }`.
  - **FIXED (with a justified deviation on the absolute-path half).** `parse_paths` now rejects any entry containing a `..` parent-dir component — the genuine directory-traversal escape vector (e.g. `src/../../etc/passwd`) — and documents why. I deliberately did **not** reject absolute paths: `diagnose` is contractually an **absolute-space API** (the diagnostics tool's `produce_outcome` and the `files edit` fold-in both relativise/absolutise around it and hand it absolute repo paths — see `crates/swissarmyhammer-tools/src/mcp/tools/diagnostics/mod.rs` "diagnose works in absolute space" and `inline_diagnostics.rs` "already-filtered, absolute, diagnosable paths"). Rejecting absolute paths would reject every legitimate follower call and break the gated integration test. New unit tests: `parse_paths_rejects_parent_dir_traversal`, `parse_paths_accepts_absolute_repo_paths`.
- [x] `crates/swissarmyhammer-leader-election/src/request_ipc.rs` — Public struct `RequestServer` does not implement `Debug`. All public types should be debuggable for observability, error messages, and testing. Add `#[derive(Debug)]` to `RequestServer` (derive if fields allow, else manual impl).
  - **FIXED.** Added `#[derive(Debug)]` to `RequestServer` (`UnixListener` and `PathBuf` are both `Debug`, so a derive suffices).
- [x] `crates/swissarmyhammer-leader-election/src/request_ipc.rs` — The reader task spawned in `RequestClient::from_stream` has deeply nested control flow (loop > match > if-let > if-let, ~5 levels), making the demux logic hard to follow. Extract the inner demux logic into a helper, e.g. `handle_response_line(line, &reader_inner, &pending) -> Result<(), IpcError>`. Flattens nesting to 3 levels and isolates the demux invariants.
  - **FIXED.** Extracted `deliver_response_line(line, &Weak<ClientInner>) -> ReaderStep` (with a small `ReaderStep { Continue, Stop }` enum). The reader loop is now flat: the empty/undecodable/dropped-client/decode-and-deliver invariants live in the helper using early-return `let-else`/`?`-style guards; the loop just matches `Stop` to break.

### Nits
- [x] `crates/swissarmyhammer-diagnostics/tests/leader_follower_request_ipc.rs` — Single-call helper `which_on_path()` wraps a simple PATH lookup and is called only once (from `rust_analyzer_available()`). Inline it to remove the single-call wrapper.
  - **FIXED.** Inlined the PATH lookup into `rust_analyzer_available()`; removed the wrapper.
- [x] `crates/swissarmyhammer-leader-election/src/request_ipc.rs` (tests) — Hardcoded handler delay base `50` should be a named constant (`const HANDLER_BASE_DELAY_MS: u64 = 50;`).
  - **FIXED.** Added `const HANDLER_BASE_DELAY_MS: u64 = 50;`.
- [x] `crates/swissarmyhammer-leader-election/src/request_ipc.rs` (tests) — Hardcoded multiplier `5` in the delay calculation should be a named constant (`const DELAY_MULTIPLIER: u64 = 5;`).
  - **FIXED.** Added `const DELAY_MULTIPLIER: u64 = 5;`.
- [x] `crates/swissarmyhammer-leader-election/src/request_ipc.rs` (tests) — Hardcoded polling delay `5` ms should be a named constant (`const CONNECT_RETRY_DELAY_MS: u64 = 5;`).
  - **FIXED.** Added `const CONNECT_RETRY_DELAY_MS: u64 = 5;`.
- [x] `crates/swissarmyhammer-leader-election/src/request_ipc.rs` (tests) — Second hardcoded polling delay `5` ms — reuse `CONNECT_RETRY_DELAY_MS`.
  - **FIXED.** Both connect-retry sleeps now reference `CONNECT_RETRY_DELAY_MS`.
- [x] `crates/swissarmyhammer-leader-election/src/request_ipc.rs` (tests) — Hardcoded timeout `2` s should be a named constant (`const TEST_TIMEOUT_SECS: u64 = 2;`).
  - **FIXED.** Added `const TEST_TIMEOUT_SECS: u64 = 2;`.
- [x] `crates/swissarmyhammer-leader-election/src/request_ipc.rs` (tests) — Second hardcoded timeout `2` s — reuse `TEST_TIMEOUT_SECS`.
  - **FIXED.** Both regression timeouts now reference `TEST_TIMEOUT_SECS`.

### Reviewer verdict & deferral judgment
- Engine counts: 1 blocker, 3 warnings, 7 nits (blocker refuted by the reviewer → effectively 0 blockers / 3 warnings / 7 nits).
- **Deferred live-MCP wiring is acceptable scope for THIS card.** The three acceptance criteria are mechanism-level (listener carries the SAH request API; leader multiplexes/demuxes by envelope id; followers satisfy diagnose/query via socket with typed not-leader error) and are all satisfied and proven by the gated integration test `crates/swissarmyhammer-diagnostics/tests/leader_follower_request_ipc.rs` (real rust-analyzer leader, follower over a real socket, 8 concurrent correlated calls mixing diagnose + textDocument/definition, plus the typed leader-pid error on connect to absent leader). Reuse is correct: it binds at the existing `socket_path()`, reuses `peek_leader_pid`, and introduces no second election or duplicate envelope/serialization mechanism.
- The DEFERRED production rewiring (leader binds/serves the socket at `server.rs` startup; `produce_outcome` in `crates/swissarmyhammer-tools/src/mcp/tools/diagnostics/mod.rs` routes the follower path through `SessionRequestClient` instead of `settled_empty()`) is genuinely separate cross-crate scope. It was flagged by the implementer but **no follow-on existed**, so the reviewer created **^4rjtgsj** ("Wire the request multiplexer into the live MCP path") depending on this card to capture it. With that logged, no acceptance criterion goes silently unmet.
- **Remaining work on this card:** address the 3 warnings + 7 nits above. Task stays in `review`.