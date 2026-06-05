---
assignees:
- claude-code
depends_on:
- 01KT7A3G6KAABN7R8Q54QKNDKR
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffdf80
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

## Review Findings (2026-06-03 22:45)

### Warnings
- [x] `crates/mirdan/src/install.rs:1578-1610` (deinit_profile) — Deinit re-resolves the *current* builtin set via `resolved_skill_names`/`resolved_agent_names` to decide what to remove, rather than removing what was actually installed. If the builtin skill/agent set drifts between the installing version and the deinitializing version (a skill renamed or dropped), `sah deinit` will silently leave the old skill/agent symlink + store entry orphaned. The deleted `LockfileCleanup` path recorded exactly what was installed, making deinit robust to set drift. This is largely inherent to the mandated "Profile is the single source of truth" design, but worth a deliberate decision: either accept it (document the version-coupling on `deinit_profile`) or have deinit also sweep store/symlink entries that look sah-managed but are no longer in the profile. RESOLVED (accepted + documented): this drift is inherent to the data-driven "Profile is the single source of truth" design — there is no per-install manifest to consult, so deinit removes exactly what the current binary's selector resolves to. Adding a sweep would reintroduce the per-install record the card mandated deleting. Documented the version-coupling in a `# Version coupling` section on `deinit_profile`'s doc comment: cross-version deinit across a builtin-set rename/drop should run with the same binary version that installed.
- [x] `crates/mirdan/src/install.rs` (init_profile/deinit_profile) — Builtin skills/agents are no longer recorded in `mirdan-lock.json` (the old `SkillDeployment`/`AgentDeployment` wrote `resolved: "builtin"` entries; `LockfileCleanup` removed them). I confirmed nothing reads those builtin entries back (`mirdan status` is path-based; uninstall of builtins goes through the profile selector, not the lockfile), so this is a safe simplification — but it is a behavior change from "verified unchanged." Confirm no external consumer (doctor output, support tooling) inspects `mirdan-lock.json` expecting builtin entries. RESOLVED (confirmed safe + documented): grepped the whole workspace for lockfile consumers. In-crate: `list.rs::discover_packages` is a filesystem scan that only consults the lockfile to enrich the `source` field of registry packages (builtins have no registry key, so they were never enriched); `info.rs::show_lockfile_info` does a `get_package` lookup that falls through to the filesystem `show_local_info` path for builtins; `sync.rs` operates on registry packages. Out-of-crate: the only non-kanban/non-docs hit is `apps/mirdan-app/src/commands.rs:12`, a doc comment on the `source` field (derived from `discover_packages`, not a builtin-entry reader) — no doctor/support tooling inspects builtin lockfile rows. Documented the deliberate "builtins are not lockfile-recorded" decision in a `# Lockfile` section on `init_profile`'s doc comment.

### Nits
- [x] `crates/mirdan/src/install.rs:859-909` (Selector) — `Selector::Profile`, `Selector::Named`, and `Selector::Single` have no production caller anywhere in this workspace; sah uses only `Selector::All`, and the variants are exercised only by unit tests in this module. They are the public API of card 2's data-driven installer, intended for the sibling CLIs (kanban-cli, code-context-cli) that deploy a profile-tagged subset — so this is expected forward-looking API on the library crate, not a defect of this card. Flagging only so card 2's review confirms those consumers actually land; if they do not, the unused variants become speculative abstraction. ACKNOWLEDGED (no change): the finding itself classifies this as expected forward-looking API on the mirdan library crate for the sibling CLIs (card 2's surface), not a defect of this card. No code change warranted; the standing note to confirm those consumers land is carried by card 2's review.