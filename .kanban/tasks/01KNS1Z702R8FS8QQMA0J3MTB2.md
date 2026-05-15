---
assignees:
- claude-code
depends_on:
- 01KNS1WP3ZEAKNNAD6G3WAGSEK
position_column: done
position_ordinal: ffffffffffffffffffffffffff9e80
project: code-context-cli
title: Implement operations executor — run code-context ops from CLI
---
## What
Create `code-context-cli/src/ops.rs` that translates parsed CLI commands into `CodeContextTool` MCP calls and prints results.

### Approach
Each CLI operation variant builds a `serde_json::Map<String, Value>` matching the MCP parameter schema, then calls `CodeContextTool::execute(args, &context).await`.

The result is a `CallToolResult` containing `Content` items. Print each content item:
- If `--json` flag: print raw JSON of the full result
- Otherwise: extract text from `Content::Text` items and print to stdout

```rust
pub async fn run_operation(
    command: &Commands,
    json_output: bool,
) -> i32
```

The function opens a `ToolContext` (same as `serve.rs`), constructs the appropriate JSON args map from the matched `Commands` variant, calls `tool.execute()`, and prints output.

### Mapping example (for all 23 operations):
```rust
Commands::Get(GetCommands::Symbol { query, max_results }) => {
    args.insert("op", json!("get symbol"));
    args.insert("query", json!(query));
    if let Some(n) = max_results { args.insert("max_results", json!(n)); }
}
Commands::Get(GetCommands::Callgraph { symbol, direction, max_depth }) => {
    args.insert("op", json!("get callgraph"));
    args.insert("symbol", json!(symbol));
    // etc.
}
// ... all 23 operations
```

Error handling: if `execute()` returns `Err`, print to stderr and return exit code 1.

## Acceptance Criteria
- [ ] `cargo check -p code-context-cli` passes
- [ ] `code-context get status` runs without panic (may report empty index)
- [ ] `code-context get symbol --query foo` runs and produces output or empty result

## Tests
- [ ] `test_get_status_builds_correct_args` — unit test that arg map has `op: "get status"`
- [ ] `test_grep_code_builds_correct_args` — verify pattern is in args map
- [ ] `test_search_symbol_builds_correct_args` — verify query and optional kind
- [ ] Run `cargo test -p code-context-cli ops` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.