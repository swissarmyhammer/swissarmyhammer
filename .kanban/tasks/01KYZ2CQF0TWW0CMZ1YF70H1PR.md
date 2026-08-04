---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz77tavvernevn9v385p53ya
  text: |-
    Swept all 135 `#[error("Capital...")]` attributes across the 12 files named in the task, plus 3 hand-rolled Display-equivalent format! strings in `content_capability_validator.rs::to_acp_error` that duplicate the enum's message text.

    Kept capitalized (per rule): all-caps acronyms (MCP, MIME, URI, SSRF, IO, ID), CamelCase identifiers (LoadSession, ResourceLink), and proper nouns (Claude, Base64 — treated as a standard/proper-noun encoding name, consistent with Git/Claude).

    Scope decision: did not rewrite construction-site String arguments passed into `AgentError::*(...)`/other enum variants (the "224 sites" in the task's Size section) — those land mid-message (after the enum's own "prefix: " text), so they do not affect the Display message's first character, which is what `builtin/validators/rust/rules/error-handling.md` and the task's "Rule to apply" section actually govern. Verified via `rg '#\[error\("[A-Z]' crates/claude-agent/src` — the only remaining acceptance-criterion check — that this is the full and correct scope.

    Fixed every test whose assertion broke as a result (found by re-running the suite, not just by inspection):
    - `src/error.rs` — 9 `assert_eq!(...to_string()...)` in `test_error_display` + `test_agent_error_to_acp_error_conversion`
    - `src/session_errors.rs` — `protocol_error.message.contains("Invalid session ID format")` → lowercase
    - `src/acp_error_conversion.rs` — 4 assertions (`test_base64_error_conversion`, `test_size_exceeded_error_conversion`, `test_security_violation_no_info_disclosure`, `test_content_processing_error_invalid_structure`, `test_content_processing_error_invalid_base64`)
    - `src/content_security_integration_tests.rs` — one `.contains(...)` updated for consistency (test also has lowercase-substring OR-fallbacks so it would not have failed, but the string no longer matched reality)
    - `src/tools.rs` — one speculative `.contains("Invalid session ID format")` OR-branch (dead in practice; `SessionSetupError::InvalidSessionId` is never constructed outside its own test) updated for correctness
    - `tests/integration/coverage_tests.rs` — `test_agent_error_server_error_display` and `test_session_id_error_invalid_display` (outside `src/`, caught only by running the suite)

    Verification:
    - `rg '#\[error\("[A-Z]' crates/claude-agent/src` → only acronym/CamelCase/proper-noun starts remain (35 lines, all expected).
    - `cargo build -p claude-agent` clean.
    - `cargo nextest run -p claude-agent` → 1087 passed, 0 failed.
    - `cargo clippy -p claude-agent --all-targets -- -D warnings` → clean.
    - `cargo check -p swissarmyhammer-validators -p swissarmyhammer-agent -p acp-conformance` (the three workspace crates that depend on claude-agent) → clean, no downstream breakage.

    step: implement
    outcome: changed
    evidence: 15 files — crates/claude-agent/src/{error.rs, session_errors.rs, content_block_processor.rs, content_security_validator.rs, acp_error_conversion.rs, base64_processor.rs, path_validator.rs, mime_type_validator.rs, content_capability_validator.rs, base64_validation.rs, session.rs, size_validator.rs, tools.rs, content_security_integration_tests.rs}, crates/claude-agent/tests/integration/coverage_tests.rs
    task: ^f70h1pr
  timestamp: 2026-08-04T20:35:37.467683+00:00
- actor: claude-code
  id: 01kz788wrxjq1wjzv99e6fhb2x
  text: |-
    ### test — green
    - evidence: `cargo nextest run --workspace` — 13501 tests run, 13501 passed, 0 failed, 0 skipped (3 slow, 2 leaky). `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean, 0 warnings.
    - also checked: no `#[ignore]` tests in the repo. Cross-checked all 124 old (uppercase) claude-agent error strings against the full workspace, and specifically against the 4 dependent crates (swissarmyhammer-agent, acp-conformance, swissarmyhammer-validators, swissarmyhammer-tools) — no stale assertions found outside claude-agent itself. All claude-agent Display impls confirmed lowercase.
    - next: none, suite is green.
  timestamp: 2026-08-04T20:43:34.557339+00:00
- actor: claude-code
  id: 01kz789xk6jbn2pra2sx4pr8tw
  text: |-
    ### test — green
    - evidence: `cargo nextest run --workspace` — 13501 tests run, 13501 passed, 0 failed, 0 skipped. `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean, 0 warnings.
    - acceptance check: `rg '#\[error\("[A-Z]' crates/claude-agent/src` returns only acronym (MCP, MIME, DoS, URI, SSRF, IO), CamelCase-identifier (LoadSession), and proper-noun (Claude) starts — matches the rule.
    - no `#[ignore]` tests found anywhere in the workspace.
    - checked swissarmyhammer-validators, swissarmyhammer-agent, and acp-conformance for stale exact-text assertions on claude-agent error messages — none found. Their `to_string()`/`contains()` assertions test each crate's own error types, not claude-agent's.
    - next: none, task is clean.
  timestamp: 2026-08-04T20:44:08.166225+00:00
- actor: claude-code
  id: 01kz78bpqjpkr3py9tg1tqd4jw
  text: |-
    ### commit — changed
    - evidence: e5ba6695f fix(claude-agent): lowercase capitalized error Display messages
    - next: ready for review

    step: commit
    outcome: changed
    evidence: e5ba6695f fix(claude-agent): lowercase capitalized error Display messages
    task: ^f70h1pr
  timestamp: 2026-08-04T20:45:06.674213+00:00
position_column: doing
position_ordinal: '8380'
title: Lowercase the capitalized error Display messages in claude-agent
---
`builtin/validators/rust/rules/error-handling.md` states: Display messages on errors are lowercase, with no trailing punctuation.

`^p4mp9n6` swept swissarmyhammer-tools, mirdan and agent-client-protocol-extras to completion, and fixed the one named claude-agent site (`tools.rs` `unknown tool: {}`). The rest of claude-agent still starts its error messages with a capital.

## Size

- 135 `#[error("Capital...")]` attributes across 12 files. The biggest are `src/error.rs` (29), `src/session_errors.rs` (27), `src/content_block_processor.rs` (16), `src/content_security_validator.rs` (13), `src/acp_error_conversion.rs` (13), `src/base64_processor.rs` (12), `src/path_validator.rs` (11).
- 224 `AgentError::*(...)` construction sites, concentrated in `src/mcp.rs`.

## Rule to apply

Lowercase the first character unless the first word is:

- an all-caps acronym — `MCP`, `JSON`, `I/O`, `ACP`, `HTTP`, `SSE`, `URL`;
- a CamelCase identifier — `LoadSession`, `HookEvaluator`;
- a proper noun — `Git`, `Claude`.

Also strip a trailing full stop.

Leave capitalized, as `^p4mp9n6` did:

- the UI titles in `src/tool_classification.rs`, and the `assert_eq!(title, "Unknown tool")` tests that pin them;
- log lines (`tracing::*`);
- `.expect()` panic text.

## Risk

claude-agent errors surface as ACP protocol error payloads. Several tests assert the exact `to_string()` — for example `src/error.rs` `assert_eq!(err.to_string(), "Permission denied: access denied")` and `src/session_errors.rs` `assert_eq!(data["details"], "Missing required fields")`. Every such test must change in the same commit.

## Acceptance

- `rg '#\[error\("[A-Z]' crates/claude-agent/src` returns only acronym, CamelCase-identifier and proper-noun starts.
- `cargo nextest run -p claude-agent` green.
- `cargo clippy -p claude-agent --all-targets -- -D warnings` clean. #bug