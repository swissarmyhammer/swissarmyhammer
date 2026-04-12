---
assignees:
- claude-code
position_column: todo
position_ordinal: b680
project: kanban-mcp
title: 'sah-cli: extract serve.rs from commands/serve/ to top-level module'
---
## What

Extract MCP serve functionality from `swissarmyhammer-cli/src/commands/serve/mod.rs` into a top-level `swissarmyhammer-cli/src/serve.rs`, matching the pattern in shelltool-cli and code-context-cli.

## Acceptance Criteria
- [ ] `swissarmyhammer-cli/src/serve.rs` exists at top level
- [ ] `commands/serve/` delegates to or is replaced by top-level module
- [ ] `sah serve` still works
- [ ] `cargo test -p swissarmyhammer-cli` passes
