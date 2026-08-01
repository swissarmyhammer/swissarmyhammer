---
assignees:
- claude-code
position_column: todo
position_ordinal: de80
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