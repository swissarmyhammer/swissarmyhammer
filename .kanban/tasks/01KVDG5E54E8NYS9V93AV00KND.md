---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvexdzmmzfddr5h4hv5z9p93
  text: 'Picked up by /finish $diagnostics, after ^x6m3jpz → done. Clean rule-of-three dedup: json_result + string_arg/bool_arg/string_array_arg are byte-identical across review/mod.rs, code_context/mod.rs, diagnostics/mod.rs. Extract to ONE shared module (op_tool_helpers.rs or under tool_registry), replace per-tool copies with imports, no behavior change. Note: the prior code_context WIP that blocked folding this into na6cvh0 is now committed, so this is clean. Guard = cargo build --workspace + tools test suite. Driving implement → test → review.'
  timestamp: 2026-06-19T03:05:15.156702+00:00
- actor: claude-code
  id: 01kvexxkve6xhptxttczpd1f98
  text: |-
    Implemented. Shared module: crates/swissarmyhammer-tools/src/mcp/op_tool_helpers.rs (pub(crate) json_result, string_arg, bool_arg, string_array_arg), declared in mcp/mod.rs. Chose op_tool_helpers.rs over tool_registry/utils — the existing utils.rs/shared_utils.rs are about template-arg conversion and response structs, not op-dispatch arg readers, and the task recommended this location.

    Copies removed: 7 total across 3 files — review/mod.rs (all 4), diagnostics/mod.rs (json_result + string_arg), code_context/mod.rs (json_result). Grep for `fn json_result|fn string_arg|fn bool_arg|fn string_array_arg` in the tools crate now hits only op_tool_helpers.rs. No other op-tools had copies (git/kanban/etc. do not).

    Were they identical? Near-identical with two trivial drifts, reconciled behavior-safe:
    1. json_result error text: code_context used "Failed to serialize result: {}" vs review+diagnostics "failed to serialize: {e}". Canonicalized on the 2-of-3 majority "failed to serialize: {e}". Text only surfaces in a to_string_pretty failure (unreachable for the typed response structs); no test/caller asserts on it (grepped).
    2. string_arg body: review `.map(|s| s.to_string())` vs diagnostics `.map(str::to_string)` — semantically identical; used str::to_string.

    Dead imports cleaned: removed unused `Content` from review/mod.rs and diagnostics/mod.rs (only used by their deleted json_result); kept Content+McpError in code_context (still heavily used). Added a unit test module (4 tests) covering all 4 helpers.

    Verification (all green): cargo build -p swissarmyhammer-tools exit 0; cargo clippy -p swissarmyhammer-tools --all-targets -- -D warnings exit 0 / 0 warnings; cargo fmt --check exit 0; cargo test -p swissarmyhammer-tools exit 0 (4 new helper tests pass, full suite green). .config/nextest.toml untouched. Adversarial double-check running.
  timestamp: 2026-06-19T03:13:47.374100+00:00
- actor: claude-code
  id: 01kvey2mv74t0whx9eqrj0ds62
  text: '/finish disposition → DONE. Pure mechanical dedup; acceptance criteria fully machine-verifiable and verified. Direct grep confirms json_result/string_arg/bool_arg/string_array_arg are defined ONLY in mcp/op_tool_helpers.rs — zero remaining per-tool copies in review/code_context/diagnostics. Full tools suite green (1087 lib + 4 new helper tests + integration + doctests, 0 failures); clippy -D warnings clean; fmt clean. Two trivial drifts reconciled behavior-safe (json_result error text → majority form, only surfaces on an unreachable to_string_pretty failure; string_arg .map(str::to_string) identical). really-done adversarial double-check returned PASS (behavioral equivalence, no remaining copies, no dead imports, visibility correct). Did NOT run a full review-engine pass: acceptance is 100% machine-verified + double-check PASS, so a review sweep would only surface tangential nits (churn avoidance per standing guidance). Next: /commit local rollback point (not pushed).'
  timestamp: 2026-06-19T03:16:32.231729+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffc780
project: diagnostics
title: Extract shared op-tool dispatch helpers (json_result, string_arg, …) across review/code_context/diagnostics
---
## What
`json_result` (serialize a value into a JSON-text `CallToolResult`) and the small arg readers (`string_arg`, `bool_arg`, `string_array_arg`) are now duplicated identically across at least three op-dispatched MCP tools:
- `crates/swissarmyhammer-tools/src/mcp/tools/review/mod.rs`
- `crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs`
- `crates/swissarmyhammer-tools/src/mcp/tools/diagnostics/mod.rs`

All three `json_result`s are byte-identical (Result<CallToolResult, rmcp::ErrorData>, since McpError = rmcp::ErrorData). Rule-of-three reached → extract to one shared module and have every op-tool import it.

## Why deferred (from na6cvh0 review)
Surfaced by the na6cvh0 review. Not folded into na6cvh0 because a correct consolidation must touch code_context/mod.rs, which had unrelated uncommitted WIP at the time; doing it as its own task keeps that change clean and reviewable.

## Plan
- Add a shared module (e.g. `crates/swissarmyhammer-tools/src/mcp/op_tool_helpers.rs` or under `tool_registry`) with `json_result`, `string_arg`, `bool_arg`, `string_array_arg`.
- Replace the per-tool copies in review, code_context, and diagnostics with imports.
- `cargo build --workspace` + the tools test suite as the guard.

## Acceptance Criteria
- [ ] One canonical `json_result` + arg readers; review/code_context/diagnostics all import them (no per-tool copies).
- [ ] tools crate builds + tests green; no behavior change.

#diagnostics