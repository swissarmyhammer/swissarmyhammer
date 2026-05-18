---
position_column: todo
position_ordinal: '9880'
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