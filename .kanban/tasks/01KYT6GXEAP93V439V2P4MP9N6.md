---
assignees:
- claude-code
position_column: todo
position_ordinal: ba80
title: Lowercase the remaining capitalized MCP error Display messages outside the kanban tool
---
`builtin/validators/rust/rules/error-handling.md` states: Display messages on errors are lowercase, with no trailing punctuation.

`^1t92gnj` lowercased every error message in the two files it touched — `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs` and `apps/kanban-cli/src/commands/serve.rs`. The same message text stays capitalized in the sibling MCP call handlers, which were out of that card's scope:

- `crates/swissarmyhammer-tools/src/mcp/server.rs` — `format!("Unknown tool: {}", name)` and `format!("Unknown tool: {}", request.name)`. A test in the same file asserts `msg.contains("Unknown tool")`, so it must change with them.
- `crates/agent-client-protocol-extras/src/test_mcp_server.rs` — `format!("Unknown tool: {}", request.name)`.
- `crates/claude-agent/src/tools.rs` — `"Unknown tool: {}"`. Note that `Unknown tool` ALSO appears there as a UI TITLE (`tool_classification.rs`, and the `assert_eq!(title, "Unknown tool")` tests). A title is not an error Display message — leave those capitalized.
- `mirdan::install::detected_agents_or_error` — `"Failed to load agents config: {e}"`, recorded as out of scope on `^1t92gnj` round 7.

## Scope

Sweep each crate for error Display messages that start with a capital. Lowercase only the ERROR messages. Leave capitalized: `InitResult::ok` success messages (already adjudicated on `^1t92gnj`), UI titles, log lines that are not failures, and `.expect()` panic text.

Update every test that pins the old casing in the same change.

## Acceptance

- A grep for `format!("Unknown tool` returns only lowercase forms, except the UI-title sites.
- `cargo nextest run` green, `cargo clippy --workspace --all-targets -- -D warnings` clean. #bug