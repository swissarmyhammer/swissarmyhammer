---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffbe80
project: kanban-mcp
title: 'sah-cli: create commands/registry.rs with Initializable pattern'
---
## What

Create `swissarmyhammer-cli/src/commands/registry.rs` using the `Initializable` + `InitRegistry` pattern from shelltool-cli and code-context-cli. Currently init/deinit is ad-hoc in `commands/install/`. Refactor to use `register_all()` with priority-ordered components.

sah-cli already has `commands/` — this adds the registry module alongside the existing command modules.

## Acceptance Criteria
- [x] `swissarmyhammer-cli/src/commands/registry.rs` exists with `register_all`
- [x] `sah init` and `sah deinit` route through the registry
- [x] `cargo test -p swissarmyhammer-cli` passes
