---
assignees:
- claude-code
position_column: todo
position_ordinal: b580
project: kanban-mcp
title: 'shelltool-cli: add commands/skill.rs, move serve/doctor/registry into commands/'
---
## What

Adopt the `commands/` directory convention in shelltool-cli, matching sah-cli's structure.

1. Create `commands/` directory with `mod.rs`
2. Move `src/serve.rs` → `src/commands/serve.rs`
3. Move `src/doctor.rs` → `src/commands/doctor.rs`
4. Move `src/registry.rs` → `src/commands/registry.rs`
5. Create `src/commands/skill.rs` — extract skill deployment from `swissarmyhammer-tools/src/mcp/tools/shell/mod.rs` into a dedicated module matching code-context-cli's skill.rs pattern

Top-level keeps: `main.rs`, `cli.rs`, `banner.rs` (infrastructure).
`commands/` gets: `serve.rs`, `doctor.rs`, `registry.rs`, `skill.rs` (command implementations).

## Acceptance Criteria
- [ ] `shelltool-cli/src/commands/` exists with serve, doctor, registry, skill
- [ ] Top-level serve.rs, doctor.rs, registry.rs removed
- [ ] `commands/skill.rs` has `ShelltoolSkillDeployment` implementing `Initializable`
- [ ] Skill deployment removed from `swissarmyhammer-tools/src/mcp/tools/shell/mod.rs`
- [ ] `shelltool serve`, `shelltool init`, `shelltool deinit`, `shelltool doctor` all still work
- [ ] `cargo test -p shelltool-cli -p swissarmyhammer-tools` passes
