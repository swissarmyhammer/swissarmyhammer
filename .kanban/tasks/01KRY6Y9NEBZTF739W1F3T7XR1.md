---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff9780
title: 'claude-agent: fix ~38 pre-existing session-storage test failures and a server::tests hang'
---
## What
`cargo test -p claude-agent` had **~45 pre-existing failures** outside the `tools` module, plus one hanging test. Confirmed pre-existing (reproduce on a clean tree with all uncommitted changes stashed) and unrelated to the plugin platform work — discovered during task 01KRXK1ZGYAHA8YSHJPM6D7503 (which fixed the 5 `tools::tests` terminal tests; these were a separate problem).

## Root Causes (multiple)

1. **`session_validation::validate_directory_permissions` mutated process cwd.** It called `std::env::set_current_dir(path)` and then tried to restore the original cwd, but the restore was a no-op (it queried `current_dir()` *after* the change and got back the path it had just changed to). Concurrent tests then raced on the process-global cwd, and `SessionManager::default_storage_path()` derives its path from `current_dir()` — so unrelated tests started seeing `Session("No storage path configured")` whenever `current_dir()` returned `Err` because the cwd had been pointed at a now-deleted `TempDir`.

2. **Tests called `SessionManager::new()` and `TerminalManager::new()` directly.** Those constructors default to `cwd/.swissarmyhammer/sessions` and have no client capabilities set, so any test that listed sessions saw real source-tree pollution from earlier runs and any terminal operation failed the `validate_terminal_capability` precondition.

3. **`PathValidator::with_blocked_paths` tests used non-canonical temp dirs.** On macOS, `/var/folders/...` is a symlink to `/private/var/folders/...` and `validate_absolute_path` canonicalizes input, so `path.starts_with(blocked)` never matched.

4. **`ResourceLink::new(name, uri)` argument order was swapped in three sites** (`content_block_processor.rs`, `content_capability_validator.rs`, `protocol_translator.rs`). The schema's signature is `(name, uri)` but tests passed `(uri, name)`, sending a non-URL through the URL validator.

5. **`-32602` is now a typed `ErrorCode::InvalidParams` variant**, not `ErrorCode::Other(-32602)` — one `session_errors` test assertion stopped matching after the schema crate added the typed variant.

6. **`test_validate_client_capabilities_with_invalid_*meta*` tried to inject a non-object meta value**, but the schema crate types `meta` as `Option<Map<String, Value>>` — a non-object cannot reach the validator anymore.

7. **Stale `Created`/`released`/string-ID assertions in `terminal_manager` tests** — terminals are now born `Running`, `release_terminal` removes the entry rather than tombstoning it, and one test used a 25-char ULID instead of 26 — so it hit `Invalid session ID format` before reaching the `Session not found` path it was trying to exercise.

8. **The hanging `server::tests::test_json_rpc_error_response_format`** — `handle_single_request` propagated `serde_json::from_value(params)?` failures up to the outer loop which just logged them. The response branch never ran, so the client waited forever for a reply that was never sent. Now `parse_params` converts the deserialize error into an `agent_client_protocol::Error::invalid_params()` so the existing error-response writer takes over. The wire-level code also now reflects the actual `e.code` (e.g. -32602 InvalidParams) instead of hardcoded -32603.

## Acceptance Criteria
- [x] `cargo test -p claude-agent` passes — zero failures, including all `terminal_manager::tests`, `session::tests`, `path_validator::tests`, `capability_validation::tests`. (696 lib + 307 integration tests + 6 doctests, all green)
- [x] `server::tests::test_json_rpc_error_response_format` completes (no hang) — fix the underlying wait, do not `#[ignore]` it. Root cause: the server silently swallowed the `?`-propagated deserialize error; now it converts to `agent_client_protocol::Error::invalid_params()` and routes through the existing error-response branch.
- [x] No assertion was weakened and no test was `#[ignore]`d to pass — every fix is either a fixture repair (canonical temp paths, isolated storage path, set capabilities) or a real production-contract fix (no-cwd-mutation permission check; serialize-error → invalid-params response).

## Tests
- [x] `cargo test -p claude-agent` — all green, with `--test-threads=1` and a hard per-test timeout to catch the hang regressing.
- [x] `cargo clippy -p claude-agent --all-targets -- -D warnings` — clean.
- [x] `cargo build --workspace` — clean.

## Notes / Scope Flag
This was deeper than a fixture-only repair. Three production-code changes were necessary:
- `crates/claude-agent/src/session_validation.rs`: replaced the `set_current_dir`-based "execute permission" probe with a non-mutating `libc::access(X_OK)` check on Unix. The original implementation was broken (the restore was a no-op even single-threaded) and was the root cause of most of the "session storage" cascade.
- `crates/claude-agent/src/server.rs`: `parse_params` helper that turns `serde_json` errors into `agent_client_protocol::Error::invalid_params()` so each method arm yields `Err(e)` into `response_result` instead of `?`-propagating out and skipping the write. Also fixed the response error code to use `e.code` instead of a hardcoded -32603 so JSON-RPC consumers can discriminate `InvalidParams` (-32602) from `InternalError` (-32603).

Test-side changes are also non-trivial: two test helpers (`create_test_session_manager`, `create_test_terminal_manager`) standardize the temp-dir storage path + capability handshake; `path_validator` tests now canonicalize the temp_dir root; `ResourceLink::new` argument order corrected at three call sites.

Not plugin-platform scope — standalone `claude-agent` tech debt. #test-failure