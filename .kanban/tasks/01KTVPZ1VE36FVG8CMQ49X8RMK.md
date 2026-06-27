---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw365rv3vxmqmgg8z9v47a2d
  text: |-
    Picked up. Research done. Current state: `register_mcp_server_at` / `unregister_mcp_server_at` / `resolve_agent_mcp_config` already exist and work (install.rs), private, reachable only via `install_profile_mcp`. They call `mcp_config::register_mcp_server`, which already dispatches JSON vs TOML by file extension via `settings::read/write_mcp_config` (the now-done TOML dep) — so codex `.codex/config.toml` composes for free. Public API convention: consumers call `mirdan::install::register_mcp_server` (pub fn in `pub mod install`), no lib.rs re-export — so promotion = make the two fns `pub` + docs.

    TDD plan: the genuine RED for a visibility change is an EXTERNAL test (in-crate `super::*` tests can call private fns, so they can't prove "callable from outside the crate"). Adding `crates/mirdan/tests/register_mcp_server_at.rs` that calls `mirdan::install::register_mcp_server_at` — compile-fails (private) until pub. Plus the card-prescribed comprehensive four-agent applier_tests (claude/cursor/codex/zed shapes at a temp root != CWD, assert no CWD writes, + unregister). nextest is the gate (skips doctests), so external test lives in tests/. Adding swissarmyhammer-common as a dev-dep for NullReporter/InitScope.
  timestamp: 2026-06-27T00:02:51.875305+00:00
- actor: claude-code
  id: 01kw36h0rf59st3mstt0vajrxx
  text: |-
    DONE (green, left in doing for review).

    Public API added:
    - `pub fn mirdan::install::register_mcp_server_at(root: &Path, server_name: &str, entry: &McpServerEntry, scope: InitScope, reporter: &dyn InitReporter) -> Vec<InitResult>`
    - `pub fn mirdan::install::unregister_mcp_server_at(root: &Path, server_name: &str, scope: InitScope, reporter: &dyn InitReporter) -> Vec<InitResult>`

    Both promoted from private `fn` (logic unchanged). Doc comments document: project/local config paths joined onto `root`; user scope uses each agent's absolute global config; JSON-vs-TOML writer selected by extension; per-agent `servers_key`/`entry_extras` honored; never reads `current_dir()`. No lib.rs re-export needed — `pub mod install` already makes `mirdan::install::*` reachable (matches `register_mcp_server`). The strategy-dispatched CWD-implicit `register_mcp_server` is untouched.

    TDD: RED = external integration test `crates/mirdan/tests/register_mcp_server_at.rs` failed to compile with E0603 (private function) → made pub → PASS. Plus card-prescribed in-crate `applier_tests` four-agent test (claude `.mcp.json` / cursor `.cursor/mcp.json` / codex `.codex/config.toml` TOML / zed `.zed/settings.json` `context_servers` with `source:custom`) registering at a temp root != a guarded CWD, asserting exact absolute command/args preserved and NOTHING written under CWD; + unregister-removes-all-four test.

    double-check (advisory) flagged a redundant dev-dep: I had added `swissarmyhammer-common` to `[dev-dependencies]`, but it is already a normal `[dependencies]` entry and normal deps ARE reachable from integration tests — verified empirically (integration test compiles/passes without it). Reverted that line; Cargo.toml now unchanged from main.

    Verification: `cargo nextest run -p mirdan` 413 passed 0 skipped; `cargo clippy -p mirdan --all-targets -- -D warnings` clean; `cargo fmt --check -p mirdan` clean.

    Files changed: crates/mirdan/src/install.rs (pub + docs + 2 tests + helper); crates/mirdan/tests/register_mcp_server_at.rs (new). Cargo.toml ends up unchanged.
  timestamp: 2026-06-27T00:09:00.431347+00:00
depends_on:
- 01KTVPYFR4JCHF6AZ651X41NFS
position_column: doing
position_ordinal: '8180'
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