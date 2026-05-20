---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffff580
title: 'Flaky test: test_list_sessions_after_new_session relies on non-monotonic ULID ordering'
---
What: `acp::server::tests::test_list_sessions_after_new_session` in `crates/llama-agent/src/acp/server.rs` (assertion at server.rs:2605) intermittently fails ~1 in 5 runs. It creates two sessions back-to-back and asserts the second sorts ahead of the first in `list_sessions` output. When both sessions are created within the same millisecond their ULIDs share the timestamp prefix and the random tails sort in arbitrary order, so "newest first" is not guaranteed.

Observed failure: left "01KRYAQFSMZ2RTKNMXNYDCY77T" vs right "01KRYAQFSM6G185Q6N3GWE3HFD" — identical 10-char timestamp prefix `01KRYAQFSM`.

Acceptance Criteria: test passes deterministically across many runs. Either the session id generation uses monotonic ULIDs, or the test does not assume strict creation-order sort for same-millisecond ids (e.g. sorts the two ids itself, or asserts on a stamped timestamp rather than ULID order).

Tests: `cargo nextest run -p llama-agent --lib acp::server::tests::test_list_sessions_after_new_session` green across 20+ consecutive runs.

Discovered while addressing review findings on task 01KRXHWBC6QN88TVWN5ZA4Z7X0; out of scope for that task (different test binary, unrelated to the with_temp_state helper).