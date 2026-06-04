---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffdd80
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

## Review Findings (2026-06-03 14:43)

Edge flip is clean and complete: skills has zero mirdan refs (only doc comments) + no mirdan dep; mirdan deps skills + templating; `cargo tree -p swissarmyhammer-skills -i mirdan` and `-p swissarmyhammer-templating -i mirdan` both report "did not match any packages" (acyclic confirmed); build green for skills/mirdan/all three CLIs; deploy tests pass; clippy clean. Callers correctly rerouted to `mirdan::install::stage_and_deploy_skill`. Only nits below.

### Nits
- [x] `crates/mirdan/src/install.rs:680` — The new `stage_and_deploy_skill` `is_safe_name` guard branch has no direct test. RESOLVED: added `test_stage_and_deploy_skill_rejects_traversal` asserting `stage_and_deploy_skill("../escape", ...)` returns `RegistryError::Validation`, exercising the rejection path through `stage_and_deploy_skill`.
- [x] `apps/{shelltool-cli,code-context-cli,kanban-cli}/src/commands/skill.rs` — `render_skill` is byte-for-byte identical across all three CLI skill modules. DEFERRED (out of scope per this card's body: "do NOT yet delete the per-app skill.rs logic (cards 3/4)"). This exact cleanup is already tracked by card 4 (`01KT7A4D44637D9Z1THZX6DASP`), whose description lists deleting the byte-identical `render_skill` copies in all three CLIs. Carried forward there; nothing to do on this card.