---
assignees:
- claude-code
depends_on:
- 01KT7A3G6KAABN7R8Q54QKNDKR
- 01KT7A3Z4FNVZX1GJCMMS65A0F
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffe180
project: mirdan-install
title: Migrate kanban desktop app to mirdan explicit-root init; delete swissarmyhammer-workspace-init
---
Retire the parallel-mechanism crate entirely.

## Background
`swissarmyhammer-workspace-init` existed ONLY because mirdan's entry points were CWD/auto-detect rooted, unsafe in the long-running multi-board kanban desktop process. Card 2 adds an explicit-root variant to mirdan's installer, removing that reason. The crate's other contents (`ProjectStructure`, `SkillDeployment`, `render_skill`, `write_skill`, `format_skill_md` re-export, `is_safe_skill_name`/`is_safe_relative_path`, `KNOWN_PROFILES`, `WORKSPACE_TOOLS`) all duplicate mirdan/skills and its copy-into-`.sah/skills/` deploy is a divergent mechanism vs mirdan's store+symlink.

## Change
- Find every caller of `swissarmyhammer-workspace-init` (kanban desktop app: `apps/kanban-app`; possibly others) and migrate them to `mirdan::install::init_profile(profile, scope, Some(root))` with the appropriate Profile.
- Resolve the deploy-semantics question explicitly: the in-process board `skill` MCP tool currently reads `<root>/.sah/skills/`. Decide whether the board reads from mirdan's store/symlinked location instead, or mirdan's explicit-root deploy targets `<root>/.sah/skills/`. EITHER WAY there must be ONE mechanism — do not keep the copy-vs-symlink fork. (Confirm what the board's skill tool actually reads and align.)
- **Delete `crates/swissarmyhammer-workspace-init` entirely** (Cargo.toml workspace member, the crate dir, and all `swissarmyhammer-workspace-init` deps in other crates' Cargo.toml).
- Move `KNOWN_PROFILES`/`WORKSPACE_TOOLS` data into mirdan (or skills) as the single profile registry if still needed.

## Done when
- `apps/kanban-app` initializes workspaces via mirdan's explicit-root installer; no CWD access; the board's skill tool reads from the single deployed location.
- `swissarmyhammer-workspace-init` no longer exists; no crate references it; workspace builds.
- One deploy mechanism for builtin skills everywhere (mirdan store+symlink or one explicit-root variant — not both).
- `cargo build --workspace` green; clippy clean; kanban-app tests pass.

Depends on the mirdan Profile installer (card 2) and the sah migration (card 3, which removes the other workspace-init consumer).

## Review Findings (2026-06-03 18:40)

Verified end-to-end: crate `crates/swissarmyhammer-workspace-init` is gone; no `workspace-init`/`workspace_init` references remain outside `.kanban/` task descriptions; `Cargo.toml`/`Cargo.lock` carry no member or dep. `cargo build -p kanban-app` green, `cargo clippy -p kanban-app --all-targets` clean, `workspace_init` integration tests (3) and `state::` unit tests (24, incl. the real `open_board` production-path test) all pass. The migration is correct: the board declares a minimal `Profile { skills: Some(Selector::Profile(\"kanban\")) }` and calls the one shared `mirdan::install::init_profile` rooted at the explicit board folder — no CWD access, single store+symlink mechanism, deploy target `<board>/.skills/`. Only documentation nits below.

### Nits
- [x] `apps/kanban-app/src/state.rs:1088` — Stale comment fixed: now references the `.skills/` deploy store created as the `.kanban` sibling, not `.sah/`.
- [x] `apps/kanban-app/src/state.rs:1151` — Doc line reworded: skills/prompts resolve "from that board's deployed `.skills/` store", removing the inconsistent `.sah/` naming.
- [x] `apps/kanban-app/src/state.rs:102` — `mcp_server` field doc updated: skills/prompts now documented to resolve from the board's `.skills/` deploy store.

All three are comment-only changes. `cargo clippy -p kanban-app --all-targets` clean.