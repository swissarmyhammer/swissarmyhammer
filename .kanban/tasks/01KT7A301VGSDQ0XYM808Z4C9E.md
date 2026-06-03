---
assignees:
- claude-code
position_column: todo
position_ordinal: '8180'
project: mirdan-install
title: Invert the skills→mirdan dependency edge
---
Foundational. Today `swissarmyhammer-skills` depends on `mirdan` for ONE reason: `deploy::write_and_deploy` (`crates/swissarmyhammer-skills/src/deploy.rs:157`) calls `mirdan::install::deploy_skill_to_agents`. Deploy is not skills' job. Removing it flips the edge so mirdan can depend on skills+templating and own install/init outright.

## Change
- **Delete `write_and_deploy` (and `validate_skill_name` if it only exists for it) from `crates/swissarmyhammer-skills/src/deploy.rs`.** Keep the pure-content helpers: `resolve_skill`, `resolve_profile_skills`, `format_skill_md`. Skills becomes deployment-free.
- **Remove `mirdan = { workspace = true }` from `crates/swissarmyhammer-skills/Cargo.toml:27`.** Confirm no other `mirdan` ref remains in skills (only deploy.rs:157 + doc comments today).
- **Add `swissarmyhammer-skills` + `swissarmyhammer-templating` deps to `crates/mirdan/Cargo.toml`** (mirdan currently deps neither). This is what lets mirdan render+deploy builtin skills.
- **Verify acyclic**: `cargo tree`/`cargo build` must stay green. Confirm skills'/templating's transitive deps (`swissarmyhammer-operations`, `swissarmyhammer-build`, `swissarmyhammer-config`) do NOT reach mirdan. If any does, surface it before proceeding.
- The current `write_and_deploy` callers (shelltool/code-context/kanban `commands/skill.rs`) will break — that's expected; they get replaced by the mirdan profile installer in later cards. For THIS card, the minimal move is: relocate the deploy step into mirdan (e.g. a `mirdan::install::stage_and_deploy_skill(name, content)` or fold into the profile installer of card 2) and point the existing callers at it so the workspace still builds. Do NOT yet delete the per-app skill.rs logic (that's cards 3/4) — just keep the build green after the edge flip.

## Done when
- `swissarmyhammer-skills` has zero `mirdan` references and no `mirdan` dep.
- `mirdan` deps `swissarmyhammer-skills` + `swissarmyhammer-templating`; workspace is acyclic.
- `cargo build --workspace` green; existing skill-deploy tests pass (callers temporarily routed to mirdan's relocated deploy).