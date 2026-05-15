---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffa680
project: kanban-mcp
title: 'sah-cli: add commands/skill.rs for explicit skill deployment'
---
## What

Create `swissarmyhammer-cli/src/commands/skill.rs` for deploying builtin skills, matching code-context-cli's skill deployment pattern. sah-cli already has `commands/` — this adds the skill module alongside the existing command modules.

Registered via `commands/registry.rs` `register_all`.

## Acceptance Criteria
- [x] `swissarmyhammer-cli/src/commands/skill.rs` exists
- [x] Registered via registry.rs `register_all`
- [x] `sah init` deploys sah skills
- [x] `cargo test -p swissarmyhammer-cli` passes

## Implementation Notes

- Extracted `SkillDeployment` (with its `Initializable` impl) + the four skill-deployment helpers (`deploy_all_skills`, `deploy_single_skill`, `render_skill_instructions`, `format_skill_md`) from `swissarmyhammer-cli/src/commands/install/components/mod.rs` into a dedicated `swissarmyhammer-cli/src/commands/skill.rs`.
- `commands::skill::SkillDeployment` is now registered from `commands::registry::register_all` (alongside the call to `install::components::register_all`), matching shelltool-cli's layout where `ShelltoolSkillDeployment` is explicitly registered in `commands/registry.rs`.
- Kept the sah-specific Liquid rendering path (via `PromptLibrary` + `TemplateContext`) because sah's builtin skills use `{% include %}` partials that the simpler `swissarmyhammer-templating` engine used by code-context-cli / shelltool-cli does not expand.
- Made `is_safe_name` and `save_lockfile_and_report` `pub(crate)` in `install/components/mod.rs` so the extracted skill module can reuse them (also used by the in-place `AgentDeployment`).
- Added unit tests covering: component name/category/priority, builtin-skill resolution, frontmatter format, metadata preservation (with round-trip parse via `skill_loader::parse_skill_md`), field omission for empty values, Liquid `{{version}}` expansion, and init/deinit result counts.
- Registry test updated to verify `skill-deployment` appears in init results.

## Verification
- `cargo test -p swissarmyhammer-cli --lib`: 447 passed, 0 failed, 0 warnings
- `cargo test -p swissarmyhammer-cli` (all suites): 447 + 426 + 11 + 1 + 231 + 16 + 1 = 1133 tests passed, 0 failed
- `cargo clippy -p swissarmyhammer-cli --all-targets -- -D warnings`: clean
- `cargo fmt -p swissarmyhammer-cli -- --check`: clean

## Review Findings (2026-04-12 14:20)

### Nits
- [x] `swissarmyhammer-cli/src/commands/registry.rs:45-47` — The inline test comment says "7 components from components::register_all (McpRegistration, ClaudeLocalScope, DenyBash, ProjectStructure, ClaudeMd, AgentDeployment, LockfileCleanup) + 1 KanbanTool + 1 SkillDeployment = 9". `KanbanTool` is actually registered by `install::components::register_all` (see line 33 there), not as a separate contribution, so `components::register_all` contributes 8 components, not 7. The arithmetic is correct (8+1=9); only the attribution in the comment is misleading. Suggested rewording: "8 components from components::register_all (the 7 installable components + KanbanTool) + 1 SkillDeployment (from commands::skill) = 9".