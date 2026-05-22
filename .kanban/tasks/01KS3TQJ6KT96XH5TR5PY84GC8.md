---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff9380
title: 'Fix flaky timeout: test_mcp_server_prompt_loading (parallel contention)'
---
**File**: `apps/swissarmyhammer-cli/tests/integration/mcp_integration.rs:43`

**Symptom**: Times out at 300s under full workspace nextest, but PASS [8.3s] in isolation.

**Root cause hypothesis**: Same as `test_mcp_server_basic_functionality` — in-process HTTP MCP server contention under heavy parallel load. Also uses `IsolatedTestEnvironment` which mutates `HOME` env var; possible cross-test interference even with the guard.

**Reproducer**:
- `cargo nextest run --workspace` -> TIMEOUT [300s]
- `cargo nextest run -E 'test(test_mcp_server_prompt_loading)'` -> PASS [8.3s]

**Suggested fix**: Mark with `#[serial_test::serial]` together with the other two failing in-process MCP server tests. Investigate whether HOME-env mutation in `IsolatedTestEnvironment` should serialize.

**Acceptance criteria**: 3 consecutive `cargo nextest run --workspace` runs complete with this test passing.

**Pre-existing**: not caused by recent UI work on the `kanban` branch.

#test-failure

---

## Root-Cause Fix Attempt (2026-05-21) — working_dir isolation

Per the unchecked Review Findings warning below, replaced the serialization + timeout-override workaround with the documented `working_dir` isolation pattern from `crates/swissarmyhammer-tools/src/mcp/test_utils.rs` (`test_client_list_tools`).

**Changes applied**:
- All three in-process MCP server tests in `mcp_integration.rs` now pass a small temp dir as the `start_mcp_server` `working_dir` arg (was `None`, which bound the server to the host monorepo and ran `startup_cleanup`'s full-repo walk on every startup):
  - `test_mcp_server_basic_functionality` -> `Some(tempfile::TempDir)`
  - `test_mcp_server_prompt_loading` -> `Some(IsolatedTestEnvironment::temp_dir())` (reuses the env it already creates — the project's isolated test system)
  - `test_mcp_server_builtin_prompts` -> `Some(tempfile::TempDir)`
- Removed all three `#[serial_test::serial(mcp_server)]` attributes and their workaround doc comments.
- Removed the `mcp_server` per-test `slow-timeout` override block from `.config/nextest.toml`.
- No Cargo.toml change (`tempfile` already a dev-dep; `serial_test` still used by other CLI tests, kept).

**Verification — isolation (the win is real)**:
`cargo nextest run -p swissarmyhammer-cli -E 'test(test_mcp_server_basic_functionality) | test(test_mcp_server_prompt_loading) | test(test_mcp_server_builtin_prompts)'` -> all 3 PASS, running CONCURRENTLY (no serial guard), wall-clock 8.07s:
- `test_mcp_server_prompt_loading` PASS [6.762s]
- `test_mcp_server_basic_functionality` PASS [8.061s]
- `test_mcp_server_builtin_prompts` PASS [8.063s]
The monorepo walk is gone; the working_dir fix is correct and is a genuine improvement.

**Verification — full workspace (the plan's premise FAILS under load)**:
`cargo nextest run --workspace` (~23.8 min, log /tmp/nextest_full_run_isolation_fix.log). 13769 tests. The three target tests still stall under full load:
- `test_mcp_server_prompt_loading` PASS [298.281s] (barely under 300s)
- `test_mcp_server_basic_functionality` TIMEOUT [300.136s]
- `test_mcp_server_builtin_prompts` TIMEOUT [300.149s]

This is a NET REGRESSION vs the prior workaround state (serial group + 600s override), where all three PASSED at 189s/234s/371s. Removing the serial guard let all three run concurrently with each other AND a large cluster of OTHER in-process MCP-server tests scheduled at the tail of the run, and 2 of 3 hit the default 300s ceiling.

**Key disconfirming evidence**: the canonical pattern test the plan cited as proof — `swissarmyhammer-tools mcp::test_utils::tests::test_client_list_tools`, which ALREADY uses `Some(tempfile::TempDir)` as working_dir — ALSO TIMED OUT [300.151s] in this same run, alongside `test_client_call_tool` [300.149s]. So the working_dir pattern alone does NOT prevent the timeout under full-workspace load. The residual stall is a workspace-wide contention among many concurrent in-process MCP-server-startup tests (cli `mcp_integration`, tools `final_http` / `rmcp_stdio_working` / `skill_e2e` / `rebuild_index_progress_notifications` / `test_utils`, kanban-app `state::tests`), not the monorepo walk.

**7 total timeouts in the full run** (5 are the pre-existing class already tracked; 2 are mine that regressed):
- `swissarmyhammer-cli mcp_integration::test_mcp_server_basic_functionality` (REGRESSED by this change)
- `swissarmyhammer-cli mcp_integration::test_mcp_server_builtin_prompts` (REGRESSED by this change)
- `swissarmyhammer-tools mcp::test_utils::tests::test_client_list_tools` (the canonical working_dir test — already times out under load)
- `swissarmyhammer-tools mcp::test_utils::tests::test_client_call_tool`
- `swissarmyhammer-tools mcp::file_watcher::tests::test_file_watcher_start_watching_replaces_previous`
- `kanban-app state::tests::test_open_board_serves_full_sah_mcp_toolset`
- `swissarmyhammer-entity watcher::tests::test_attachment_remove_emits_event`

**Status: BLOCKED — awaiting user guidance.** The directed plan removed the serial guard and timeout override on the explicit assertion they would be redundant once working_dir was set. The evidence falsifies that assertion: under full-workspace load the working_dir fix is necessary but NOT sufficient, and removing the guards causes a real regression (PASS -> TIMEOUT). Re-adding either guard would directly contradict the user's explicit instruction, so I am stopping to ask rather than improvising. The code/config edits are in place; the working_dir improvement itself is sound. The Review Findings warning is left UNCHECKED pending this decision.

## Review Findings (2026-05-21 14:30)

Re-review of the two NEW changes (third-sibling `#[serial_test::serial(mcp_server)]` on `test_mcp_server_builtin_prompts` + the `.config/nextest.toml` per-test `slow-timeout` override). Both prior checklist items are genuinely resolved: all three siblings now carry the named-group attribute with accurate doc comments, and the override's `test(...)` filter is verified narrow — `cargo nextest list` against the filter resolves to exactly the three intended tests and nothing else (no substring over-capture; confirmed no other test names contain those substrings), and the config parses cleanly. The delivered fix is correct, minimal, and uses the repo's own override mechanism. The new finding below is a forward-looking architecture observation, not a regression — the change does not make anything worse and per this task's scope should be tracked separately, NOT re-opened here.

### Warnings
- [ ] `apps/swissarmyhammer-cli/tests/integration/mcp_integration.rs:26,78,124` — The 371s serialized cost is largely self-inflicted and avoidable; the 10-minute ceiling masks a cheaper root-cause fix that already exists in this codebase. All three target tests call `start_mcp_server(..., None)` (the 4th arg, `working_dir`, is `None`), so the in-process server binds to the host monorepo and runs `startup_cleanup` — walking and hashing every file in a very large repo — on every server startup. That walk, not just cross-workspace queue contention, is why these run 189–371s instead of the ~7–8s they take in isolation. The sibling tests in `crates/swissarmyhammer-tools/src/mcp/test_utils.rs:62-71` already solve exactly this: they pass a `tempfile::TempDir` as `working_dir` specifically "so the server doesn't bind to the host monorepo — prevents `startup_cleanup` from walking/hashing it and lets multiple server tests run in parallel without a CWD serial guard." Adopting that pattern here would likely restore these tests to seconds and make both the serialization and the 10-minute timeout bump unnecessary. Suggested follow-up: pass `Some(tempfile::TempDir)` as `working_dir` to the three `start_mcp_server` calls and re-evaluate whether the serial group + timeout override can be dropped. Forward-looking only — do not re-open this task; file as a separate follow-up.

UPDATE (2026-05-21, root-cause fix attempt): the working_dir change above WAS applied as suggested. It fixed the monorepo walk (isolation: 8s, concurrent) but did NOT make the serial guard / timeout override redundant under full-workspace load — `test_client_list_tools` (which already uses this exact pattern) also times out under load. See "Root-Cause Fix Attempt" section. The assumption that the walk is the sole cause was incorrect; there is additional workspace-wide concurrency contention. Checkbox left UNCHECKED pending user decision.