---
assignees:
- claude-code
depends_on:
- 01KMNGJW411YFMVR9K78JT7EHK
position_column: done
position_ordinal: ffffffffffe780
title: Real Initializable impl for ShellExecuteTool
---
Replace `impl_empty_initializable!(ShellExecuteTool)` with a real impl in swissarmyhammer-tools/src/mcp/tools/shell/mod.rs.\n\n## init(scope)\n1. Create `.shell/config.yaml` from builtin template if missing\n2. Deny Bash — read `.claude/settings.json`, add `\"Bash\"` to `permissions.deny`, write back (inline JSON manipulation, ~15 lines)\n3. Resolve shell skill from `swissarmyhammer_skills::SkillResolver`, render through `swissarmyhammer_templating` (expand `{{version}}`), deploy via `mirdan::install::deploy_skill_to_agents`\n4. Update lockfile\n\n## deinit(scope)\n1. Remove shell skill via `mirdan::install::uninstall_skill` (must be pub first)\n2. Remove `Bash` from `permissions.deny`\n3. Remove `.shell/` config directory\n\n## Files\n- swissarmyhammer-tools/src/mcp/tools/shell/mod.rs\n- swissarmyhammer-tools/Cargo.toml (add deps: swissarmyhammer-templating, swissarmyhammer-skills, mirdan)\n\n## Acceptance\n- init creates config, denies Bash, deploys skill\n- deinit reverses all three\n- Idempotent — calling init twice is safe\n- is_applicable returns true for Project and Local scopes only