---
assignees:
- claude-code
depends_on:
- 01KNS10MMDVZG731XKM390C682
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffb080
project: kanban-mcp
title: 'kanban-cli: add commands/skill.rs for explicit skill deployment'
---
## What

Create `kanban-cli/src/commands/skill.rs` for deploying/removing the builtin `kanban` skill. Model on `code-context-cli`'s skill deployment pattern (serde_yaml_ng frontmatter, metadata preservation, template rendering).

Exports `KanbanSkillDeployment` implementing `Initializable` (priority 20). Imported by `commands/registry.rs`.

## Acceptance Criteria
- [x] `kanban-cli/src/commands/skill.rs` exists with `KanbanSkillDeployment`
- [x] `cargo check -p kanban-cli` passes
- [x] Metadata preservation tested

## Tests
- [x] Unit test: `KanbanSkillDeployment::name()` returns correct name, priority 20
- [x] Unit test: builtin kanban skill resolves and metadata preserved through render+format