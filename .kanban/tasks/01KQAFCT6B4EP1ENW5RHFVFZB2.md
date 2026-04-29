---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff8c80
title: 'In-process validator MCP server: emit detailed tracing into .avp/log'
---
When `AvpContext` starts the validator MCP server in-process (`avp-common/src/context.rs::resolve_validator_mcp_config` → `start_mcp_server_with_options`), the server's tracing goes to the same global `tracing` subscriber that writes `.avp/log`, but the **content** of what's traced today is sparse. We can see the listener bind, MCP client connect, and per-tool-call duration, but not enough to debug what's actually happening when a per-rule validator session goes off the rails (cf. today's 16:06–16:19 Stop-hook run where `no-magic-numbers` started, made one `read_file`, then went silent for 70+ seconds and never completed).

## What's missing

Concrete gaps observed in `.avp/log` for the 16:06 Stop-hook run:

1. **Tool list response** — there's exactly one `Discovered N tools from MCP client` line per session (`9672: Discovered 4 tools from MCP client`), but no list of which tools or their schema sizes. When debugging \"why didn't the model call X?\", knowing whether X was even advertised matters. Log the tool names (and optionally a hash of the schema) at session-create time.

2. **Full tool args** — call sites log `args=N` (a count) and the request handler shows `path=...` for `read_file` specifically, but `glob_files`/`grep_files`/`code_context` don't surface their args at all in the per-tool-call info line. Need every tool invocation to log its arguments at info level (with a JSON string truncated to ~512 bytes if huge), e.g. `tool_call tool=grep_files args={...}`.

3. **Full tool response (truncated)** — `tool_call complete duration_ms=N error=false` is the only post-call line. We never see what the model actually got back. Add an info-level line per call with `result_bytes=N preview=\"...first 256 chars...\"`. Without this, we can't tell whether a hung session is the model rejecting a tool result or something else.

4. **Session lifecycle** — three errors today were `rmcp::transport::streamable_http_server::tower: Failed to close session XXX: Session error: Session service terminated` at 16:11:19, 16:13:18, 16:14:23. Why? The current logs don't tell us when those sessions were *opened*, who held them, why they got terminated mid-flight. Add tracing for: session created (with caller info if available), session closed (clean vs forced), session terminated (with cause). Match the log lines so the lifecycle of one session can be `grep`d end-to-end by `session_id`.

5. **Server lifecycle** — `Validator agent in-process MCP server bound; agent will use this endpoint` at startup is good. Add the matching shutdown line on `Drop`/`shutdown()`, ideally with `bound_for_seconds=NN total_requests=NN total_errors=NN`. Right now the in-process server's lifetime is invisible after startup.

6. **Per-request timing breakdown** — currently `duration_ms=9` is total. For long calls (`code_context: get callgraph` can take seconds), break that into `parse_ms / dispatch_ms / handler_ms / response_ms` so we can tell whether the time was in the model, the handler, or serialization.

## Where to add the tracing

- `swissarmyhammer-tools/src/mcp/unified_server.rs` — `start_mcp_server_with_options` and the axum router setup. Add `info!` for server start (already there), shutdown (missing), and a request middleware that logs `method tool_name session_id` on every JSON-RPC request.
- `swissarmyhammer-tools/src/mcp/server.rs` (or wherever `serve_inner:tool_call` is logged today) — extend the existing `tool_call` span to include `args` (truncated) and a post-call `tool_call complete` line with `result_bytes` + `preview`.
- The session-lifecycle errors come from inside `rmcp` crate, but we should add a thin wrapper layer in `unified_server.rs` that observes session create/close events and re-logs them with the validator-server context (`server_url`, `agent_mode`).

## Constraints

- **Same `.avp/log`** — do not introduce a separate sink. The whole point is one file to grep. The in-process server already shares the global tracing subscriber set up in `avp-common`; verify this still holds for any new `tracing` calls.
- **Truncation** — tool args/responses must be truncated at info level. Default 512 bytes for args, 256 bytes for preview. Allow opting into full-payload logging at trace level via `RUST_LOG=swissarmyhammer_tools::mcp=trace`.
- **Secret hygiene** — the truncation function must not split mid-UTF-8 boundary (use `floor_char_boundary`). Same risk as task `01KQ8CXYMBGN1VTV4S89FGQYCA` Warning #2: do not log the *content* of `read_file` responses or `Write` inputs at info — preview is fine for diagnostics, full content is only at trace.
- **Performance** — argument JSON serialization should not happen at all when the log level is below info (use `tracing::enabled!`). Same for response previews. The validator server is on the hot path of every per-rule run; allocating a 256-byte string per call is fine, but allocating a multi-KB args dump and then discarding it is not.

## Acceptance

After this lands, a `.avp/log` from a single Stop-hook run can answer all of these via `grep`:

1. \"Which tools were exposed to the validator agent?\" — one line per session listing names.
2. \"What did rule X actually call?\" — every tool call has a line showing tool name, full args (truncated), session id.
3. \"What did the tool return?\" — every completed call has a preview of its response.
4. \"Why did session abc-123 die?\" — session lifecycle lines tagged with `session_id` show open → activity → close/terminate.
5. \"Did the in-process MCP server shut down cleanly when avp exited?\" — one log line per process showing the bound-for duration and request count.

## Pairs with

- `01KQ35MHFJQPMEKQ08PZKBKFY0` — already establishes the in-process server. This task adds the visibility into it.
- The new task being filed alongside this one for `validator result` log lines missing on Stop. That task needs *this* one's logging to even diagnose what's happening per rule.

#avp #tracing #observability

## Review Findings (2026-04-28 14:19)

Mode: task. Scope: `swissarmyhammer-tools/Cargo.toml`, `swissarmyhammer-tools/src/mcp/mod.rs`, `swissarmyhammer-tools/src/mcp/server.rs`, `swissarmyhammer-tools/src/mcp/unified_server.rs`, `swissarmyhammer-tools/src/mcp/tracing_util.rs` (new). Compiles clean (`cargo check -p swissarmyhammer-tools`); 10 `tracing_util` unit tests pass.

Acceptance criteria checklist:

- [x] **#1 \"Which tools were exposed to the validator agent?\"** — `event=tools_listed` info-level line in `list_tools` carries `tools=name1,name2,...` and `tool_count`. Note: fires on every `tools/list` call (typically once per session, but could repeat); not strictly per-session-create.
- [x] **#2 \"What did rule X actually call?\"** — `tool_call args` info-level line carries `tool=`, `args_preview=` (UTF-8-safe truncation to 512 bytes), and the parent span carries `session_id=`. JSON serialization is correctly gated by `tracing::enabled!(Level::INFO)` and `Level::TRACE`.
- [x] **#3 \"What did the tool return?\"** — `tool_call complete` info-level line carries `duration_ms`, `error`, `result_bytes`, and `preview=` (UTF-8-safe 256-byte truncation). Trace-level emits the full payload in a separate `tool_call result (full payload)` line.
- [x] **#4 \"Why did session abc-123 die?\"** — Now COMPLETE. The middleware tracks `session_open` (first sight), `session_close` (DELETE observed — clean vs. forced via `cause` field), and `session_terminate` (non-success status on a session-bearing request). Each line carries `session_id` so the lifetime of one session can be grepped end-to-end. The `initialize` handler now emits `event=session_initialized` (distinct from `session_open`) so the schema is unambiguous.
- [x] **#5 \"Did the in-process MCP server shut down cleanly when avp exited?\"** — `event=server_shutdown` info-level line in `McpServerHandle::shutdown` carries `bound_for_seconds`, `bound_for_ms`, `total_requests`, `total_errors`, `total_sessions`, `signal_sent`, `connection_url`. Idempotent (early `take()` on `shutdown_tx`). Now backed by a `Drop` impl that fires the same line on implicit drop with `dropped=true`.

Constraints:

- [x] Same `.avp/log` (single global subscriber): all new tracing uses `tracing::info!` / `tracing::debug!` / `tracing::trace!` / `tracing::warn!` macros only; no new subscribers, layers, or sinks. ✓
- [x] UTF-8-safe truncation: `truncate_utf8_for_log` walks back from `max_bytes` until a `is_char_boundary` is true. The substitute for the unstable `floor_char_boundary` is correct and well-tested (10 unit tests, including 4-byte emoji, 3-byte CJK, 2-byte Latin-1, empty, zero-budget, and exact-budget cases). ✓
- [x] `tracing::enabled!` gating for serialization: args path correctly skips serialization when info+trace are both disabled. Preview path now computes when info OR trace is enabled (no longer dependent on info-level enabling for trace-only subscribers).
- [x] Truncation budgets met: 512 args / 256 preview at info; full payload at trace.
- [x] Secret hygiene at info level: at info, args and result both go through truncation. For the validator surface (read-only tools only — `read_file`, `glob_files`, `grep_files`, `code_context`), arg content is paths/patterns and is fine. The same `call_tool` handler runs for the **full** server (`sah serve`) where `write_file`/`edit_file` ops carry file content as args — at info level, that content is truncated to 512 bytes and logged. The task constraint phrases this as \"preview is fine for diagnostics, full content is only at trace\" so a 512-byte preview is technically within the constraint, but it is closer to the edge than the prose implies. Worth confirming this matches intent.

Findings:

- [x] **MEDIUM — Session close/terminate lifecycle missing (criterion #4 not fully met).** RESOLVED. The `request_observer` middleware now emits `event=session_close` on observed DELETE requests (with a `cause` field distinguishing `client_delete` from `delete_failed`) and `event=session_terminate` on any non-success response on a session-bearing non-DELETE request, with the HTTP `status` and `cause` (canonical reason) fields. Every line carries `session_id=...` so the per-session lifetime is greppable end-to-end. New regression test `test_request_observer_session_lifecycle_events` asserts open→terminate→close fire exactly once for the right session ids.

- [x] **MEDIUM — Per-request timing breakdown missing (\"What's missing\" #6 not addressed).** RESOLVED. `tool_call complete` now carries `duration_ms` (total) plus `parse_ms`, `dispatch_ms`, `handler_ms`, `response_ms` so a slow call's bottleneck is visible at info level without re-running.

- [x] **LOW — Duplicate `event=session_open` log lines.** RESOLVED. The `McpServer::initialize` handler now emits `event=session_initialized` instead of duplicating `session_open`. The middleware retains the unique `session_open` event on first-sight of a `mcp-session-id`. `grep 'event=session_open' | wc -l` now matches the actual count of distinct sessions.

- [x] **LOW — `shutdown()` doc comment mentions `Drop` but no `Drop` impl exists for `McpServerHandle`.** RESOLVED. Added a `Drop` impl that emits the same `event=server_shutdown` line with `dropped=true` when the explicit shutdown path was never taken. Idempotent: when `shutdown()` already ran, `Drop` sees `shutdown_tx == None` and emits nothing.

- [x] **LOW — Trace branch in `call_tool` depends on info-level enabling for preview.** RESOLVED. The preview/result_bytes computation is now gated by `enabled!(INFO) || enabled!(TRACE)` so a custom subscriber that selectively enables trace-only on a target still gets a populated `result_full=` line.

- [x] **LOW — No tests assert the new log output.** RESOLVED. Added two log-output assertion tests in `unified_server::tests`: `test_shutdown_emits_server_shutdown_event` (asserts `event=server_shutdown` fires once on explicit shutdown with the right counters, and is idempotent), and `test_request_observer_session_lifecycle_events` (asserts `session_open`, `session_terminate`, `session_close` each fire exactly once for the right session ids, with correct `total_requests`/`total_errors` counters). Both tests use a scoped `tracing-subscriber` layer (`set_default` returning a guard) with a custom `MakeWriter` that captures formatted lines into a shared buffer for verbatim assertion.

- [x] **LOW — Args JSON serialization at info level still allocates the full string before truncation.** RESOLVED. Added `serialize_json_bounded` to `tracing_util.rs` — a streaming serializer with a `BoundedWriter` that stops accepting bytes once a soft cap is reached while still counting the would-be size. The `call_tool` info path now uses it for args, capping allocation at `MAX_ARGS_BYTES_INFO` (512 B) regardless of the underlying value's size. Four new unit tests cover the bounded writer (short/long/multi-byte/zero-cap cases).

Overall: the truncation primitive and its tests are excellent; criteria #1, #2, #3, #5 are met cleanly; criterion #4 is the load-bearing gap. Recommending fixes for the medium findings and at least one log-output assertion test.

## Fix Round (2026-04-28 — second pass)

All review findings resolved:

- Session lifecycle now logs `session_open` / `session_close` / `session_terminate` per-session in `unified_server.rs::request_observer`.
- Initialize handler renamed to `session_initialized` (distinct schema from `session_open`).
- `tool_call complete` now reports per-phase timing (`parse_ms`, `dispatch_ms`, `handler_ms`, `response_ms`) alongside the total.
- `Drop` impl on `McpServerHandle` emits the shutdown summary even when callers forget to invoke `shutdown()`.
- Preview computation now triggers on info OR trace (decoupled).
- Bounded JSON serializer caps args allocation at the truncation budget.
- Two new tests in `unified_server::tests` (`test_shutdown_emits_server_shutdown_event`, `test_request_observer_session_lifecycle_events`) plus four new tests in `tracing_util::tests` covering `serialize_json_bounded`.

Build + clippy + tests: `cargo build -p swissarmyhammer-tools --all-targets` clean, `cargo clippy -p swissarmyhammer-tools --all-targets -- -D warnings` clean, `cargo test -p swissarmyhammer-tools --lib mcp::tracing_util` 14/14 pass, `cargo test -p swissarmyhammer-tools --lib mcp::unified_server` 33/33 pass. Full crate: 1028 pass + 1 pre-existing flake (`mcp::test_utils::tests::test_client_call_tool` — same failure on pristine `git stash`'d tree, unrelated to this change).