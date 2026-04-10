---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffff9980
title: '[warning] run_serve returns Result<(), String> instead of anyhow::Result'
---
**File**: code-context-cli/src/serve.rs (run_serve function)\n\n**What**: `run_serve` returns `Result<(), String>`, using `.map_err(|e| e.to_string())` to flatten errors. This loses the error chain -- when the server fails to start, the caller gets a flat string with no source chain, no backtrace, and no ability to match on specific error types.\n\n**Why**: Per Rust review guidelines, application code should use `anyhow::Result<T>` with `.context(\"what we were doing\")` on every `?`. A bare stringified error like \"connection refused\" without context is unhelpful for debugging.\n\n**Note**: This pattern is inherited from `shelltool-cli/src/serve.rs` which has the same issue. It is a pre-existing pattern, but the new crate is an opportunity to improve it.\n\n**Suggestion**: Change to `pub async fn run_serve() -> anyhow::Result<()>` and use `.context(\"starting MCP stdio server\")` and `.context(\"MCP server terminated unexpectedly\")`.\n\n**Verify**: `cargo check -p code-context-cli` and confirm error messages include context." #review-finding