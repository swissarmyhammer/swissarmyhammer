---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffd80
project: ai-panel
title: Add a per-launch auth token to the in-process WebSocket ACP agent
---
## What

The in-process loopback `ws://127.0.0.1:<port>` ACP agent server (`apps/kanban-app/src/ai/agent_ws.rs`) currently accepts ANY local connection. The accept loop performs no origin or auth check, so any local process that discovers the OS-assigned ephemeral port could connect and drive an in-process agent. The only mitigation today is loopback-only binding, which keeps it off the network but does not isolate it from other local processes.

Harden the channel with a per-launch auth token so only the app's own webview can connect.

## Acceptance Criteria
- [x] `AgentWebSocketServer::bind_with` (or `bind`) mints a fresh, cryptographically random per-launch token.
- [x] `ai_start_agent` includes the token in the `wsUrl` handed to the webview — e.g. as a `ws://127.0.0.1:<port>?token=<secret>` query parameter or as a WebSocket subprotocol.
- [x] The WebSocket server rejects any connection that does not present the correct token (close the connection / fail the upgrade before running the ACP protocol).
- [x] The TypeScript ACP client passes the token when opening the connection (reads it from the `wsUrl` returned by `ai_start_agent`).
- [x] Unit/integration test: a connection presenting the wrong/no token is rejected; a connection presenting the correct token completes `initialize`.

## Context
This is the deferred follow-up to review finding on `01KRRN3SP5D1H63TQ8HM7SQZ1F` ("Model selection and the AI agent endpoint command surface"). That task wired the server into Tauri startup but, per the reviewer's accepted resolution, deferred the token handshake to this separate tracked task rather than expanding its scope. The `run()` doc comment in `agent_ws.rs` references this task id as the place the token work is tracked.

## Tests
- [x] `cargo test -p kanban-app` is green.
- [x] `cargo clippy -p kanban-app --all-targets -- -D warnings` is clean. #security

## Implementation Notes

### Token mechanism: query parameter
The per-launch token travels as a `token` query parameter of the `ws://` URL — `ws://127.0.0.1:<port>/?token=<secret>`. Chosen over a `Sec-WebSocket-Protocol` subprotocol because:
- The TypeScript ACP client (`connectAcpStream`) opens whatever URL it is handed verbatim via `new WebSocket(url)`, and `aiPanelConnectFactory` passes `endpoint.wsUrl` straight through. A query param is carried automatically with **zero TypeScript code change** — the browser includes the query string in the HTTP upgrade request line. A subprotocol would have required the TS client to also send a matching `Sec-WebSocket-Protocol` header.
- The Rust accept path switched from `tokio_tungstenite::accept_async` to `accept_hdr_async`, whose `Callback` (an `FnOnce(&Request, Response) -> Result<Response, ErrorResponse>`) inspects the upgrade request's URI query string and rejects a bad token with HTTP `401 Unauthorized` — failing the handshake before any agent is built or any ACP protocol runs.

### RNG
`rand` 0.9 via `rand::rng()` + `random_range` over a 64-symbol URL-safe alphabet (`A-Za-z0-9-_`), 43 characters (~258 bits of entropy). This is the workspace's established token pattern (`mirdan`'s OAuth `state`, `swissarmyhammer-web`'s privacy module). `rand::rng()` is a ChaCha-based CSPRNG, so the token is cryptographically unguessable. `rand` was added as a direct `kanban-app` dependency (`{ workspace = true }`); it was already a workspace dependency.

### Comparison method
The workspace has no `subtle` crate, so a small `constant_time_eq` helper performs the token check: it XOR-accumulates every byte (folding length difference into the accumulator) so its running time does not depend on where a mismatch occurs — no timing side channel that would leak how many leading characters a guess got right.

### Token lifetime
The token is minted once per `AgentWebSocketServer` instance in `bind_with` and exposed via `AgentWebSocketServer::token()`. `RunningAgents::start` reads it and builds the `wsUrl`. Each board re-selection / app launch mints a fresh secret.

### Integration test
`apps/kanban-app/tests/agent_ws.rs` extended (TDD — tests written and seen to fail before the implementation): `connection_with_correct_token_completes_initialize`, `connection_with_wrong_token_is_rejected`, `connection_without_token_is_rejected`, `each_launch_mints_a_distinct_non_empty_token`. The pre-existing `initialize_round_trip` helper now appends the server's token. Plus inline unit tests in `agent_ws.rs` for `mint_token`, `constant_time_eq`, and `token_from_request`.

## Review Findings (2026-05-18)

Task-mode security review. Scope: `apps/kanban-app/src/ai/agent_ws.rs`, `src/ai/models.rs`, `Cargo.toml`/`Cargo.lock` (`rand` 0.9.2 — confirmed CSPRNG, `ThreadRng` backed by `ReseedingRng<ChaCha12Core, OsRng>` / `rand_chacha 0.9.0`), `tests/agent_ws.rs`. The three TS files (`acp-stream.ts`, `ai-panel.tsx`, `ai-panel-container.tsx`) are whole new files from the parent `ai-panel` branch, not changes from this task; their only token-relevant content is doc comments and the pass-through `wsUrl: string` field — `connectAcpStream` opens the URL verbatim, so the token rides along with zero TS code change as claimed. `cargo test -p kanban-app --test agent_ws` green (11 tests, incl. the 3 token tests); `cargo clippy -p kanban-app --all-targets -- -D warnings` clean.

Verified good: the token gate runs inside `accept_hdr_async`'s callback and fails the upgrade with HTTP 401 before `create_agent` / `serve_agent` are ever reached — no code path lets an unauthenticated connection reach the agent. `token_from_request` parsing is robust against missing / empty / multi / malformed `token` (all reduce to a non-matching value, none accidentally pass). `constant_time_eq` folds length into the accumulator, runs `0..max(len)`, never early-returns on length or first diff — genuinely timing-safe for this use. The 3 integration tests genuinely fail if the gate is removed. `#[allow(clippy::result_large_err)]` on `token_gate` is justified — the `Result<Response, ErrorResponse>` shape is dictated by `tokio-tungstenite`'s `Callback` trait. Token is minted per launch with ~258 bits of entropy.

### Major
- [x] **Per-launch token is leaked to the OS log.** `ai_start_agent` (`apps/kanban-app/src/ai/models.rs:389-394`) logs `ws_url = %ws_url` at `info` level, and `ws_url` is `ws://{addr}/?token={secret}` (built at `models.rs:302`). The GUI tracing subscriber (`init_gui_tracing` in `main.rs:108-112`) routes every `tracing` event into the macOS unified log via `tracing_oslog::OsLogger`, and the CLI subscriber defaults to `info` too — so the secret is written, at `info`, to a persistent log that any local user/process can read (`log show --predicate 'subsystem == "com.swissarmyhammer.kanban"'`). This defeats the exact threat model this task exists to close: a co-resident local process that cannot guess the token can simply read it out of the log. `agent_ws.rs` was careful to keep its rejection log line a static string with no token interpolation — `models.rs` undoes that care. Fix: log a redacted URL (drop or mask the `token` query parameter) in `ai_start_agent`'s `tracing::info!`, e.g. log `addr` / a `ws://{addr}/` form instead of the full secret-bearing URL. Re-audit any other site that logs `ws_url` or `AgentEndpoint`.

  **Resolution (2026-05-18):** Added a pure helper `redact_token_in_url` in `agent_ws.rs` (alongside `TOKEN_QUERY_PARAM`, where the URL/token mechanics already live; `pub(crate)` so `models.rs` can use it). It splits the URL on `?`, then replaces every `token` query-parameter *value* with `<redacted>` while preserving scheme/host/port/path and all non-secret parameters. `ai_start_agent`'s `tracing::info!` now logs `ws_url = %redact_token_in_url(&ws_url)` instead of the raw URL — the loopback address still identifies the endpoint, the secret never reaches a log sink. Swept the whole diff for other leak sites: `models.rs:392` was the *only* place a token-bearing URL reached a log. `RunningAgent` / `AgentEndpoint` both derive `Debug` but no `tracing`/`println!`/`dbg!`/`eprintln!` call ever formats either one; `agent_ws.rs`'s rejection log (line ~282) and listener log (~216) carry no token; `RunningAgents::stop`/`stop_all` log only the board path/count; `cli.rs`'s `println!`s serialize unrelated command results. No other leak site exists. New unit tests `redact_token_in_url_masks_the_secret`, `redact_token_in_url_masks_token_among_other_params`, `redact_token_in_url_passes_through_url_without_query` assert the secret value does not survive redaction.

### Minor (non-blocking — note for the author)
- [x] `constant_time_eq` truncates the length XOR to `u8` (`(a.len() ^ b.len()) as u8`). A length difference that is an exact multiple of 256 would XOR to a low byte of 0 and not register in `diff` via the length term. It is not exploitable here — the byte loop runs to `max(len)` and zero-pads, so a real content mismatch is still caught, and the expected token is a fixed 43-char URL-safe string an attacker cannot match with NUL padding — but folding the full `usize` (e.g. `diff |= ((a.len() ^ b.len()) != 0) as u8`, or accumulate the XOR without truncation) would remove the theoretical gap and better match `subtle`'s contract. Optional hardening.

  **Resolution (2026-05-18):** `constant_time_eq`'s accumulator is now a `usize`, not a `u8` — `let mut diff = a.len() ^ b.len();` folds the *full-width* length difference with no truncation, and each per-byte XOR is widened via `usize::from(x ^ y)` before being OR-ed in. A length delta that is an exact multiple of 256 can no longer vanish from the low byte. Still genuinely constant-time: no early return on length or first diff, the loop still runs `0..max(len)`. New unit test `constant_time_eq_detects_length_delta_multiple_of_256` compares a 1-byte vs 257-byte all-zero input (the all-zero longer input rules out the byte loop catching it incidentally) and asserts they compare unequal — this test fails on the old `as u8` form.

Outcome: both findings resolved. `cargo build -p kanban-app` clean, `cargo test -p kanban-app` green (130 bin unit tests incl. the 3 redaction + length-delta tests, `agent_ws` integration suite 15 tests incl. the 3 token tests), `cargo clippy -p kanban-app --all-targets -- -D warnings` clean. Re-review.