---
position_column: done
position_ordinal: ee80
title: Extract SkillDeployment as Initializable component
---
## What

Extract `install_skills_via_mirdan()` from monolithic `init.rs` into a standalone `SkillDeployment` struct implementing `Initializable`.

- `init`: Resolves builtin skills, renders Liquid templates, deploys to `.skills/` store via mirdan, creates agent symlinks, updates lockfile
- `deinit`: Removes deployed skills, cleans symlinks, updates lockfile
- Priority: 30 (after project structure and tool-specific dirs)
- `is_applicable`: Always true (all scopes)

**Files:**
- NEW: `swissarmyhammer-cli/src/commands/install/components/skill_deployment.rs`
- EDIT: `swissarmyhammer-cli/src/commands/install/components/mod.rs`
- EDIT: `swissarmyhammer-cli/src/commands/install/init.rs` — remove `install_skills_via_mirdan()`, `render_skill_instructions()`, `format_skill_md()`

## Acceptance Criteria
- [ ] `SkillDeployment` implements `Initializable`
- [ ] `init()` deploys same skills as current `install_skills_via_mirdan()`
- [ ] Liquid template rendering preserved
- [ ] Lockfile entries written correctly
- [ ] `deinit()` cleans up skills and lockfile
- [ ] Old functions removed from `init.rs`

## Tests
- [ ] `cargo test -p swissarmyhammer-cli` passes
- [ ] Manual: `sah init` deploys skills to `.skills/`, lockfile updated