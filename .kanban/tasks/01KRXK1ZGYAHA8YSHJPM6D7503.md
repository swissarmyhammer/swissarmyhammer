---
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8880
title: Fix 5 pre-existing claude-agent terminal tests failing in tools::tests
---
crates/claude-agent/src/tools.rs (test module)

Five `tools::tests` unit tests fail. Confirmed PRE-EXISTING via `git stash` — they fail identically with no working-tree changes, and are unrelated to the swissarmyhammer-entity / ToolCallContent boxing work.

Failing tests:
- test_acp_terminal_create_invalid_session_id (tools.rs:3146) — assertion failed: error.contains("Invalid session ID format")
- test_acp_terminal_create_nonexistent_session (tools.rs:3171) — assertion failed: error.contains("Invalid session ID format") || error.contains("Session not found")
- test_acp_terminal_create_minimal_parameters (tools.rs:3112) — Result::unwrap() on Err: Protocol("No client capabilities available. Client must send initialize request with capabilities.")
- test_acp_terminal_create_with_all_parameters (tools.rs:3062) — same Protocol error
- test_terminal_method_handler (tools.rs:3325) — same Protocol error

Root cause: the terminal test setup does not provide client capabilities / a valid session ID, so terminal creation rejects the request. Either the tests need to send an initialize request with capabilities first, or the fixture/handler setup regressed.

What I tried: verified failures reproduce with all uncommitted changes stashed (baseline). Out of scope for the entity-boxing verification task; filed for follow-up. #test-failure

## Resolution

Root cause (production contract change, not a test-only regression):
`git log -L` on `terminal_manager.rs` shows commit `e9cae392a "rebase main"` added a
new `validate_terminal_capability().await?` precondition as the FIRST step of
`TerminalManager::create_terminal_with_command` (the "0. Check terminal capability"
step). Before that commit the method went straight to session-ID validation. The
capability check returns `Protocol("No client capabilities available. Client must
send initialize request with capabilities.")` whenever the `TerminalManager` was
constructed via `TerminalManager::new()` without `set_client_capabilities` having
been called.

The 5 failing tests build a bare `Arc::new(TerminalManager::new())` and call
`create_terminal_with_command` directly. They were never updated for the new
precondition, so all 5 now fail on the capability gate — and because that gate runs
before session-ID validation, even the invalid/nonexistent-session tests fail on the
capability error instead of reaching their intended `"Invalid session ID format"` /
`"Session not found"` assertions.

This is a production-contract change that the direct-`TerminalManager` test fixtures
were never updated for. The tests' intent is correct; they just need to satisfy the
new handshake. Handler-routed terminal tests (e.g. `test_terminal_create_and_write`)
already pass because `create_test_handler_with_permissions` sets capabilities on the
`ToolCallHandler`.

Fix (test setup only — no production code changed, no assertion weakened):
Added a shared async helper `create_test_terminal_manager()` next to the existing
`create_test_handler_with_permissions` helper in the `tools::tests` module. It builds
an `Arc<TerminalManager>` and calls `set_client_capabilities` with
`ClientCapabilities::new().fs(...).terminal(true)` — the same capability handshake the
handler helpers already perform. The 5 tests now use this helper instead of
`Arc::new(TerminalManager::new())`. No assertions were changed; no test was
`#[ignore]`d. The invalid/nonexistent-session tests now pass the capability gate and
genuinely exercise session-ID validation, asserting the real error strings.

Verification:
- `cargo test -p claude-agent --lib tools::tests` → `test result: ok. 50 passed;
  0 failed; 0 ignored` — all 5 formerly-failing tests pass, no other `tools::tests`
  test regressed. Clean-tree baseline (changes stashed) for the same module is
  `45 passed; 5 failed` (exactly the 5 target tests), proving the fix.
- `cargo clippy -p claude-agent --all-targets -- -D warnings` → zero warnings.
- `cargo fmt` applied. `cargo build --workspace` → Finished, clean.

The implementer also surfaced ~38 OTHER pre-existing `claude-agent` failures
(`terminal_manager::tests`, `session::tests`, `path_validator::tests`,
`capability_validation::tests` — `Session("No storage path configured")` /
`os error 22` storage-dir errors) plus a pre-existing hang in
`server::tests::test_json_rpc_error_response_format`. Confirmed pre-existing via
`git stash`; none in the `tools` module; none touched by this change.

## Scope decision (orchestrator)
This task is scoped to the **5 `tools::tests` terminal tests** named above — they are
fixed and verified. The ~38 unrelated `claude-agent` failures + the `server::tests`
hang are a separate session-storage problem, now tracked as task
`01KRY6Y9NEBZTF739W1F3T7XR1`. This task's verification gate is therefore the
`tools::tests` module (the 5 tests pass, nothing in that module regressed), NOT the
whole-crate `cargo test -p claude-agent`.

## Acceptance Criteria
- [x] The 5 named `tools::tests` terminal tests pass.
- [x] No other `tools::tests` test regressed.
- [x] No assertion weakened, no test `#[ignore]`d.
- [x] `cargo clippy -p claude-agent --all-targets -- -D warnings` and `cargo build --workspace` clean.
- [x] The ~38 unrelated pre-existing failures are filed separately (task 01KRY6Y9NEBZTF739W1F3T7XR1).