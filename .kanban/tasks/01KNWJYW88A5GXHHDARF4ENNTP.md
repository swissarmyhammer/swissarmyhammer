---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffe180
title: Expand shelltool-cli registry tests to cover init/deinit success and global paths
---
shelltool-cli/src/registry.rs:51-54, 71, 100-104, 118-121, 132, 150-156, 160-168

Coverage: 100/131 (76.3%)

Uncovered lines (grouped):
- 51-54: `Err` branch of `load_agents_config` in `init`
- 71: `global { agent_global_mcp_config(...) }` arm in `init` (currently only Project scope is tested, which hits the `else` branch)
- 100-104: `Err(e)` arm of `register_mcp_server` (warning path)
- 118-121: `Err` branch of `load_agents_config` in `deinit`
- 132: `global { agent_global_mcp_config(...) }` arm in `deinit`
- 150-156: `Ok(true)` arm of `unregister_mcp_server` (successful removal)
- 160-168: `Err(e)` arm of `unregister_mcp_server` (warning path)

The existing tests only call `init(&InitScope::Project, ...)` and `deinit(&InitScope::Project, ...)` and assert `results.len() == 1`. They don't drive through detected agents or reach the actual register/unregister branches (because the test env has no detected agents pointing at real config files in a tempdir).

**What to test:**

1. **Global scope branch (lines 71 and 132):** Call `init(&InitScope::User, &NullReporter)` and `deinit(&InitScope::User, &NullReporter)`. These traverse the `if global { ... }` arms regardless of whether any agents are detected.

2. **Success path for register/unregister (lines 150-156):** Requires a tempdir with a fake agent config file in place, PLUS a way to make `mirdan::agents::get_detected_agents` return that agent. Two approaches:
   - **Approach A:** If `mirdan` has a test-helper for injecting detected agents, use it.
   - **Approach B:** Use `tempfile::TempDir` + `env::set_current_dir(temp)` + create the expected `.claude.json` / `.mcp.json` files by hand, then call `init` with `InitScope::Project` and verify a file got written. Then call `deinit` and verify it got removed (`Ok(true)` arm).

3. **`load_agents_config` error branches (lines 51-54, 118-121):** Hard to force without filesystem manipulation. Attempt: set `HOME` to a path where `.config/mirdan/...` does not exist and is not creatable (e.g. `/dev/null/nope`). **Low priority — skip if it requires test hacks.**

4. **`register_mcp_server` / `unregister_mcp_server` error branches (lines 100-104, 160-168):** Make the target config file unwritable (chmod 0444 on a tempfile). **Low priority — skip if it requires filesystem tricks.**

Tests go in the existing `#[cfg(test)] mod tests` block in registry.rs. Prioritize (1) and (2). #coverage-gap