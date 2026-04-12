---
assignees:
- claude-code
depends_on:
- 01KNS10MMDVZG731XKM390C682
position_column: todo
position_ordinal: b480
project: kanban-mcp
title: 'kanban-cli: add commands/skill.rs for explicit skill deployment'
---
## What

Create `kanban-cli/src/commands/skill.rs` for deploying/removing the builtin `kanban` skill. Model on `code-context-cli`'s skill deployment pattern (serde_yaml_ng frontmatter, metadata preservation, template rendering).

Exports `KanbanSkillDeployment` implementing `Initializable` (priority 20). Imported by `commands/registry.rs`.

## Acceptance Criteria
- [ ] `kanban-cli/src/commands/skill.rs` exists with `KanbanSkillDeployment`
- [ ] `cargo check -p kanban-cli` passes
- [ ] Metadata preservation tested

## Tests
- [ ] Unit test: `KanbanSkillDeployment::name()` returns correct name, priority 20
- [ ] Unit test: builtin kanban skill resolves and metadata preserved through render+format
