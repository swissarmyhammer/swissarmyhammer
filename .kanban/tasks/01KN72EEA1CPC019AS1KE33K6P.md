---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffd980
title: 'Kanban CLI: output entities as YAML instead of JSON'
---
## What

The `kanban` CLI binary (`kanban-cli/src/main.rs`) outputs entity results as JSON via `serde_json::to_string_pretty` (line 124-125). It should output YAML instead, matching the `sah` CLI which already uses `serde_yaml_ng::to_string` for tool output (`swissarmyhammer-cli/src/mcp_integration.rs:240-250`).

### Change

In `kanban-cli/src/main.rs`, function `execute_kanban_operation` (line 107), replace:

```rust
let output = serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string());
```

with:

```rust
let output = serde_yaml_ng::to_string(&result).unwrap_or_else(|_| result.to_string());
```

And add `serde_yaml_ng = { workspace = true }` to `kanban-cli/Cargo.toml` dependencies.

### Files to modify

- `kanban-cli/Cargo.toml` — add `serde_yaml_ng = { workspace = true }`
- `kanban-cli/src/main.rs:124-125` — switch `serde_json::to_string_pretty` to `serde_yaml_ng::to_string`

## Acceptance Criteria

- [ ] `kanban task list` outputs YAML (key-value pairs with `:` separator, not `{}`/`[]` JSON syntax)
- [ ] `kanban board get` outputs YAML
- [ ] `kanban task add --title "test"` outputs the created entity in YAML
- [ ] Output is valid YAML parseable by `serde_yaml_ng::from_str`

## Tests

- [ ] Update `swissarmyhammer-cli/tests/kanban_cli_tests.rs` — verify output does NOT contain `{` as first non-whitespace character (i.e., not JSON)
- [ ] Existing `extract_id` helper in test already parses YAML-style `id:` lines — confirm all existing tests pass with the new output format
- [ ] Run: `cd kanban-cli && cargo test` — all tests pass
- [ ] Run: `cargo test -p swissarmyhammer-cli --test kanban_cli_tests` — all integration tests pass