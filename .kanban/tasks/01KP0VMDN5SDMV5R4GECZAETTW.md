---
assignees:
- claude-code
position_column: todo
position_ordinal: b980
project: kanban-mcp
title: 'sah-cli: add commands/skill.rs for explicit skill deployment'
---
## What

Create `swissarmyhammer-cli/src/commands/skill.rs` for deploying builtin skills, matching code-context-cli's skill deployment pattern. sah-cli already has `commands/` — this adds the skill module alongside the existing command modules.

Registered via `commands/registry.rs` `register_all`.

## Acceptance Criteria
- [ ] `swissarmyhammer-cli/src/commands/skill.rs` exists
- [ ] Registered via registry.rs `register_all`
- [ ] `sah init` deploys sah skills
- [ ] `cargo test -p swissarmyhammer-cli` passes
