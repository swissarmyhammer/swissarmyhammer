---
assignees:
- claude-code
depends_on:
- 01KT7A301VGSDQ0XYM808Z4C9E
position_column: todo
position_ordinal: '8280'
project: mirdan-install
title: 'mirdan: Profile manifest + one init/deinit installer'
---
The single source of init/install logic. After card 1, mirdan can depend on skills+templating, so it owns the whole installer.

## Profile manifest (data — the only thing that differs per consumer)
Define a `Profile` in mirdan declaring what a CLI/app installs:
- `mcp_server`: the served tool's MCP registration (name + serve command/args), or none.
- `skills`: selector — `All | Profile(name) | Named(&[&str]) | Single(&str)` (subsumes the per-app skill sets; "profile" here = the existing skill-profile filter, e.g. `kanban`).
- `agents`: which subagents to pack (selector, same shape).
- sah-only flags so sah is "just a bigger profile" not a special case: `statusline`, `preamble` (CLAUDE.md), etc. — make these declarative profile fields, not bespoke code.

## One init/deinit
`mirdan::install::init_profile(profile, scope, root?)` / `deinit_profile(...)` that, in priority order:
1. registers `mcp_server` via the existing `register_mcp_server` applier (strategy-aware),
2. renders the profile's builtin skills with Liquid (`swissarmyhammer-templating` + the partial library) and deploys them via the existing **store+symlink** `deploy_skill_to_agents` (NOT copy-into-.sah/skills),
3. deploys the profile's agents via `deploy_agent_to_agents`,
4. applies statusline/preamble when the profile declares them (reuse `settings::*` / `status::preamble_*`).
- **Explicit-root variant**: accept an optional `root: &Path` so the long-running kanban desktop process never touches CWD (this is the ONLY real reason workspace-init existed). Add root-explicit deploy/store entry points as needed so nothing reads `current_dir()`.
- Rendering: skills stay deployment-free (card 1); mirdan owns render→format→deploy. One renderer (Liquid), no simple-template variant.

## Done when
- `Profile` type + `init_profile`/`deinit_profile` exist in mirdan, fully data-driven (no per-consumer branches).
- Builtin skills render via Liquid and deploy via store+symlink through this path.
- Explicit-root operation works with no CWD access.
- Unit/integration tests: a sample profile installs (skills symlinked, MCP registered, agents deployed) and deinits cleanly; an explicit-root install targets the given root.
- `cargo build --workspace` green; clippy clean.

Depends on the edge inversion (card 1).