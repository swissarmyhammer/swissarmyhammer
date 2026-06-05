---
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffdc80
project: agent-builtins
title: 'Fix test-isolation race: serve_time_bash_deny vs per_client_tool_composition share MIRDAN_AGENTS_CONFIG'
---
## Symptom
Running both filters in one process — `cargo test -p swissarmyhammer-tools --test tools_tests -- per_client_tool_composition serve_time_bash_deny` — flakily fails `serve_time_bash_deny::llama_client_triggers_no_deny`. Each file passes in isolation.

## Root cause
`crates/swissarmyhammer-tools/tests/integration/serve_time_bash_deny.rs` redirects the **process-global** `MIRDAN_AGENTS_CONFIG` env var via an RAII guard and marks its tests `#[serial]`. But `crates/swissarmyhammer-tools/tests/integration/per_client_tool_composition.rs` runs **non-serial** and its Claude handshake (`claude_client_gets_shared_plus_shell_not_agent_tools`) triggers the serve-time native-deny path, which reads mirdan's agents config. When the two run concurrently in the same test binary, the non-serial test clobbers/reads `MIRDAN_AGENTS_CONFIG` mid-test, so the llama deny-list assertion sees a Claude config and a non-empty deny list.

## Fix options (pick one)
- Mark the Claude-handshake tests in `per_client_tool_composition.rs` `#[serial]` too (any test that can trigger the mirdan deny path must serialize on the shared env var), OR
- Make `serve_time_bash_deny` not depend on a process-global env var (inject the agents-config path through the serve options instead of `MIRDAN_AGENTS_CONFIG`).

Prefer the second (remove the global env-var coupling) per the test-isolation-raii principle: don't let env-var globals leak across tests.

## Notes
- Pre-existing; not a regression. Surfaced while implementing card 01KT57ED4DRNCZV0DXSWTT427A (per-host tool matrix coverage). NOT a production bug — the production deny path is correct; this is purely test cross-contamination.
- Default full-suite `cargo nextest`/`cargo test` runs (which run the whole binary, letting serial ordering and process-per-test isolation apply) do not exhibit this; it only appears with the combined name filter above.