---
assignees:
- claude-code
position_column: todo
position_ordinal: a280
project: agent-builtins
title: Serve the `files` write/edit replacement to Claude Code and drop the edit_redirect PreToolUse hook (deny alone closes the surface)
---
## What

`sah init` closes Claude Code's native write surface but never serves a replacement, so the model writes files via `shell` heredocs. It also installs a `PreToolUse` redirect hook that is harmful: it fires even where the `files` replacement isn't mounted (the host's own write attempts, nested contexts), producing a dead-end redirect to a tool that isn't there.

**Root cause (verified in source):**

1. `sah_profile()` sets `edit_redirect: true` (`apps/swissarmyhammer-cli/src/commands/profile.rs:45`). On `sah init`, mirdan writes `permissions.deny: ["Edit","Write","MultiEdit"]` **plus** a `PreToolUse` hook into Claude's `.claude/settings.json` that denies the native mutators and prints "use the `files` MCP tool (op \"edit file\" / \"write file\")" — see `edit_redirect_command` / `edit_redirect_hook_group` / `desired_edit_redirect_fragment` at `crates/mirdan/src/install.rs:1569-1618`.
2. But the unified `files` MCP tool is `ToolCategory::Agent` (`crates/swissarmyhammer-tools/src/mcp/tools/files/mod.rs:139-144`). The primary `sah serve` instance (`compose_per_client = true`) composes its advertised tool list per connecting host via `Host::serves` (`crates/swissarmyhammer-tools/src/mcp/host.rs:80-86`), which returns `false` for `Agent`-category tools for **every** host — including Claude. So `files` is stripped from the list Claude Code sees (`list_tools_for_host`, `crates/swissarmyhammer-tools/src/mcp/server.rs:2180-2197`).
3. Net: Claude's native `Edit`/`Write`/`MultiEdit` are denied AND the `files` replacement it is told to use is never advertised → with no write tool, the model falls back to `shell` heredocs (the `shell` tool is `Replacement{native:"Bash"}`, which **is** served to Claude).

**Precedent — the `shell` tool already does this correctly with NO hook:** `shell` is `ToolCategory::Replacement { native: "Bash" }` (`crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:536`) → served to Claude (`Host::serves(Replacement) == true` for Claude, `host.rs:84`) AND its native `Bash` is denied at serve time via plain `permissions.deny` (`apply_serve_time_native_deny` → `replacement_natives()` → `mirdan::install::deny_tool`, `server.rs:1061-1107`). No `PreToolUse` hook is involved — the deny alone closes the native surface. The `files` write surface should follow the same pattern.

**Fix (two parts, one concern — make the closed write surface usable via a served replacement, with deny-only blocking):**

1. **Serve the replacement to Claude.** Reclassify the unified `files` tool from `ToolCategory::Agent` to `ToolCategory::Replacement { native: "Edit" }` in `files/mod.rs:139-144`, so `Host::serves` advertises it to Claude (mirroring `shell`). Llama/validator paths are unaffected — `create_agent_tools_server` / `create_validator_server` register file tools explicitly and serve verbatim (`compose_per_client = false`), independent of category.
2. **Drop the redirect hook; keep the deny.** Remove the `PreToolUse` hook from the `edit_redirect` fragment (`desired_edit_redirect_fragment`, `install.rs:1609-1618`) and the now-unused hook builders (`edit_redirect_command` 1569, `edit_redirect_hook_group` 1591, `EDIT_REDIRECT_MATCHER` 1560, `HOOKS_PRETOOLUSE_POINTER`). Keep `permissions.deny: ["Edit","Write","MultiEdit"]` (the `EDIT_REDIRECT_DENY_TOOLS` constant + the deny apply path). Blocking the native tools via `permissions.deny` is sufficient to close the surface; the hook is redundant and breaks wherever the replacement isn't mounted.

**Optional consolidation (only if it stays ≤5 files; otherwise a follow-up):** aligning with the `agent-builtins` direction "move the deny from `sah init` to serve-time", extend `ToolCategory::Replacement` to carry multiple natives (`native: &'static str` → slice; `tool_registry.rs:1090-1093`, `replacement_natives` `1315-1325`, `shell/mod.rs:536` `"Bash"` → `["Bash"]`) so `files` declares `["Edit","Write","MultiEdit"]` and `apply_serve_time_native_deny` issues the full deny, letting `edit_redirect` be retired entirely. Prefer the minimal two-part fix above to honor sizing.

- [ ] Flip `FilesTool::category()` (`files/mod.rs:139-144`) `Agent` → `Replacement`.
- [ ] Update the two in-file assertions that expect `Agent` (`files/mod.rs:576,583`).
- [ ] Remove the `PreToolUse` hook from `desired_edit_redirect_fragment` + delete the unused hook builders; keep `permissions.deny`.
- [ ] Add a host-surface regression test (Claude sees `files`, Llama via primary serve does not).

## Acceptance Criteria
- [ ] `McpTool::category(&FilesTool::new())` returns `ToolCategory::Replacement { .. }` (not `Agent`).
- [ ] Building the full registry via `register_file_tools` and calling `registry.list_tools_for_host(Host::Claude)` yields a tool named `"files"`.
- [ ] `registry.list_tools_for_host(Host::Llama)` and `Host::Other` do NOT include `"files"` via the primary serve (llama still gets it through its in-process agent-tools mount — unchanged).
- [ ] `desired_edit_redirect_fragment()` contains `permissions.deny` with `Edit`/`Write`/`MultiEdit` and **no** `hooks.PreToolUse` entry; the installed Claude settings have the deny but no edit-redirect hook.
- [ ] Edit/Write/MultiEdit remain denied for Claude (write surface stays closed) with `files` now advertised as the replacement.
- [ ] The validator server (`create_validator_server`) and agent-tools server (`create_agent_tools_server`) advertised tool sets are unchanged.

## Tests
- [ ] Update the category assertions at `crates/swissarmyhammer-tools/src/mcp/tools/files/mod.rs:576,583` to expect `ToolCategory::Replacement { .. }`.
- [ ] Add a test in `crates/swissarmyhammer-tools/src/mcp/host.rs` (next to the `serves` tests at `host.rs:133-151`) or `tool_registry.rs`: build a registry with `register_file_tools`, assert `list_tools_for_host(Host::Claude)` contains `"files"` and `list_tools_for_host(Host::Llama)` does not.
- [ ] Update the mirdan `edit_redirect_tests` (`crates/mirdan/src/install.rs:6337+`) so the fragment/idempotency tests assert `permissions.deny` is present and `hooks.PreToolUse` is **absent** (these currently assert the hook is installed — they must flip).
- [ ] Run `cargo test -p swissarmyhammer-tools files host serves replacement` and `cargo test -p mirdan edit_redirect` — all green; existing `Host::serves` tests (`host.rs:133-151`) and shell `Replacement{native:"Bash"}` tests (`tool_registry.rs:3424+`) must stay green.

## Workflow
- Use `/tdd` — write the failing host-surface test (`files` visible to `Host::Claude`) and the updated mirdan deny-only fragment test first, watch them fail, then make the category + fragment changes to pass.