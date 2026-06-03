---
assignees:
- claude-code
depends_on:
- 01KT7A3G6KAABN7R8Q54QKNDKR
position_column: todo
position_ordinal: '8380'
project: mirdan-install
title: Migrate sah init/deinit to a Profile (delete bespoke components)
---
Prove the abstraction: sah must be "just a bigger profile," not a special case.

## Change
- Define sah's `Profile`: all tools (the shared SAH MCP server registration), all builtin skills, all builtin agents, plus `statusline: true` and `preamble: true`.
- Replace `apps/swissarmyhammer-cli/src/commands/registry.rs::register_all` + `commands/install/components/mod.rs` component graph with a single `mirdan::install::init_profile(sah_profile, scope)` / `deinit_profile` call.
- **Delete the bespoke Initializable components now subsumed**: `SkillDeployment` (`commands/skill.rs:44`), the `render_skill_instructions`/`write_skill_contents`/`deploy_single_skill` helpers (`commands/skill.rs`), `ProjectStructure` (`install/components/mod.rs` — the CWD/git-root one that delegated to workspace-init), `McpRegistration`, `AgentDeployment`, `Statusline`, `ClaudeMd`, `LockfileCleanup` — to the extent each becomes a declarative profile field rather than a hand-written component. Anything that genuinely cannot be expressed as profile data is a signal the Profile type (card 2) is missing a field — go back and add it, don't keep bespoke code.
- Keep `KanbanTool`/tool lifecycle registration only where it's a real tool-init concern not covered by the profile's mcp_server.
- Bash-deny is NOT here (serve-time, sticky — agent-builtins).

## Done when
- `sah init`/`sah deinit` run entirely through `mirdan::install::init_profile`/`deinit_profile` with a declared Profile; no bespoke per-step Initializable code remains in the CLI for skill/agent/mcp/statusline/preamble.
- Behavior is unchanged: same skills/agents/MCP/statusline/preamble installed, same scopes — verified against the prior behavior (existing install/doctor tests stay green).
- `cargo build --workspace` green; clippy clean.

Depends on the mirdan Profile installer (card 2).