---
assignees:
- claude-code
depends_on:
- 01KT7A3G6KAABN7R8Q54QKNDKR
position_column: todo
position_ordinal: '8480'
project: mirdan-install
title: Migrate shelltool, kanban-cli, code-context to Profiles
---
The three tool CLIs become pure profile declarations — no init logic of their own.

## Per-CLI profile
- **shelltool**: `Profile { mcp_server: shelltool, skills: Single("shell"), agents: none }`.
- **kanban-cli**: `Profile { mcp_server: kanban, skills: Profile("kanban"), agents: <kanban agents if any> }`.
- **code-context**: `Profile { mcp_server: code-context, skills: Named(["code-context","lsp"]), agents: none }`.

## Delete (now subsumed by mirdan's init_profile)
- The byte-identical `render_skill` copies: `apps/shelltool-cli/src/commands/skill.rs:33`, `apps/kanban-cli/src/commands/skill.rs:33`, `apps/code-context-cli/src/commands/skill.rs:32`.
- The `deploy_shell_skill`/`deploy_kanban_skills`/`deploy_single_skill`+`run_skill` pipelines and the `*SkillDeployment` Initializable impls (`shelltool .../skill.rs:81`, `kanban .../skill.rs:99`; code-context's orphan free fn).
- **code-context's hand-rolled MCP registration**: `apps/code-context-cli/src/commands/registry.rs::resolve_agent_targets` (33) + the register/unregister loops (100-158). Replaced by the profile's mcp_server → `register_mcp_server` applier (this also fixes its silently-missing Claude local-scope / InitScope::Local handling).
- Each app's `commands/registry.rs::register_all` collapses to "build my Profile, call mirdan init/deinit."
- Keep only genuine tool-lifecycle bits (e.g. CodeContextTool's `.code-context/` dir + `.gitignore`, KanbanTool's `.kanban/` merge drivers) that aren't install-of-an-agent concerns — but route their MCP registration through the profile.

## Done when
- `shelltool init/deinit`, `kanban init/deinit`, `code-context init/deinit` each run through `mirdan::install::init_profile`/`deinit_profile` with a declared Profile; zero `render_skill`/`SkillDeployment`/hand-rolled-MCP code remains in these apps.
- code-context MCP registration goes through the strategy-aware applier (gains local-scope handling).
- Each CLI installs the same artifacts as before (same skills, same MCP server) — verified.
- `cargo build --workspace` green; clippy clean.

Depends on the mirdan Profile installer (card 2).