---
assignees:
- claude-code
position_column: todo
position_ordinal: be80
project: kanban-mcp
title: 'code-context-cli: move serve/doctor/registry/skill into commands/'
---
## What

Adopt the `commands/` directory convention in code-context-cli, matching sah-cli's structure.

1. Create `commands/` directory with `mod.rs`
2. Move `src/serve.rs` → `src/commands/serve.rs`
3. Move `src/doctor.rs` → `src/commands/doctor.rs`
4. Move `src/registry.rs` → `src/commands/registry.rs`
5. Move `src/skill.rs` → `src/commands/skill.rs`
6. Move `src/ops.rs` → `src/commands/ops.rs` (if it's command logic)

Top-level keeps: `main.rs`, `cli.rs`, `banner.rs` (infrastructure).
`commands/` gets: `serve.rs`, `doctor.rs`, `registry.rs`, `skill.rs` (command implementations).

## Acceptance Criteria
- [ ] `code-context-cli/src/commands/` exists with serve, doctor, registry, skill
- [ ] Top-level serve.rs, doctor.rs, registry.rs, skill.rs removed
- [ ] All existing commands still work
- [ ] `cargo test -p code-context-cli` passes
