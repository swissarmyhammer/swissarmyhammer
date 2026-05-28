---
assignees:
- claude-code
position_column: todo
position_ordinal: '9080'
title: shell execute_command/get_lines tests deadlock under parallel nextest
---
crates/swissarmyhammer-tools: mcp::tools::shell::execute_command::tests::* and mcp::tools::shell::get_lines::tests::* (17 tests total in this family)

Symptom: every test that actually spawns a subprocess hangs and hits the 300s nextest timeout when run in parallel. Confirmed reproducible even in isolation of the family (run all 39 shell tests together in parallel -> TIMEOUT), but ALL 39 PASS with --test-threads=1 (38s total). A single test alone also passes in ~2.3s.

Root cause (strongly indicated): shared mutable global state in the shell tool (process registry / shell-session singleton) deadlocks/contends under concurrent access. This is the classic parallel test-isolation bug.

Fix direction: serialize the affected tests with #[serial_test::serial] (or give each test its own isolated process registry / session), per the project test-isolation convention. Do NOT add a global --test-threads=1 flag.

NOTE: pre-existing, NOT introduced by the current llama-agent test-only branch (which touches no swissarmyhammer-tools code). Surfaced by the full `cargo nextest run --workspace` run. #test-failure