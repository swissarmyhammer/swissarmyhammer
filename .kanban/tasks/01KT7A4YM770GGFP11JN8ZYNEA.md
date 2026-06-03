---
assignees:
- claude-code
depends_on:
- 01KT7A3G6KAABN7R8Q54QKNDKR
- 01KT7A3Z4FNVZX1GJCMMS65A0F
position_column: todo
position_ordinal: '8580'
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