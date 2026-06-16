---
assignees:
- claude-code
position_column: todo
position_ordinal: aa80
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