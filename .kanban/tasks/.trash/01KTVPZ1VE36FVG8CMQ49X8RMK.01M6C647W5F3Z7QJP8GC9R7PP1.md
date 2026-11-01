---
assignees:
- claude-code
depends_on:
- 01KTVPYFR4JCHF6AZ651X41NFS
position_column: todo
position_ordinal: '9e80'
project: mirdan-install
title: 'mirdan: public root-aware MCP registration API (promote register_mcp_server_at)'
---
## What
A root-explicit MCP registration function already exists but is private: `register_mcp_server_at(root, server_name, entry, scope, reporter)` at `crates/mirdan/src/install.rs:1655` (with `unregister_mcp_server_at` at `:1691` and the path resolver `resolve_agent_mcp_config` at `:1636`). It iterates mirdan-detected agents (`for_each_detected_agent`, ultimately `get_detected_agents` in `crates/mirdan/src/agents.rs:237-254`), resolves each agent's project MCP config against the passed `root` (never `current_dir()`), and writes via `mcp_config::register_mcp_server` with the agent's `servers_key` + `entry_extras` (so Zed's `{"source":"custom"}` is honored). Today it is only reachable through `install_profile_mcp` (`install.rs:1613`) when `init_profile` is called with an explicit root.

Promote `register_mcp_server_at` and `unregister_mcp_server_at` to `pub`, with doc comments explaining the relationship to the strategy-dispatched CWD-implicit `pub fn register_mcp_server` (`install.rs:3014`), which stays unchanged. The kanban-app GUI ("Expose this board to your agent") will call this from a process whose CWD is `/` and read-only, so the contract "never reads current_dir()" must be documented and test-enforced.

- [ ] Make `register_mcp_server_at` / `unregister_mcp_server_at` `pub` with docs (root semantics, project-scope rooting, user scope uses absolute global paths, no `current_dir()` reads)
- [ ] Re-export from the crate root if `install` module visibility requires it (match how `register_mcp_server` is exposed)
- [ ] Tests covering the four representative agents at a root that is NOT the process CWD (see Tests)

## Acceptance Criteria
- [ ] `mirdan::install::register_mcp_server_at` is callable from outside the crate
- [ ] With a fake agents config (in-crate `MirdanConfigGuard` from `crates/mirdan/src/test_support` or the `MIRDAN_AGENTS_CONFIG` env override, `agents.rs:140`) whose detect probes always fire, registering at an explicit temp root writes, under that root and not under the CWD:
  - `.mcp.json` (Claude Code shape, `mcpServers` key)
  - `.cursor/mcp.json` (`mcpServers` key)
  - `.codex/config.toml` (TOML, `mcp_servers` key — requires the TOML-writer task)
  - `.zed/settings.json` (`context_servers` key with `"source": "custom"` merged from `entry_extras`)
- [ ] The registered entry preserves the exact `command` (absolute path) and `args` passed in
- [ ] Existing `register_mcp_server` (CWD/strategy path, `install.rs:3014`) behavior and tests unchanged

## Tests
- [ ] New tests in `crates/mirdan/src/install.rs` `applier_tests` (pattern: `write_generic_agents_config` + `MirdanConfigGuard`, `install.rs:3074-3110`): a four-agent fake agents YAML mirroring claude-code/cursor/codex/zed `mcp_config` shapes from `agents_default.yaml`; assert each expected file exists under the temp root with the right shape, and that nothing was written relative to the process CWD
- [ ] Test that `unregister_mcp_server_at` removes the entries it registered
- [ ] `cargo test -p mirdan` passes with 0 failures

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.