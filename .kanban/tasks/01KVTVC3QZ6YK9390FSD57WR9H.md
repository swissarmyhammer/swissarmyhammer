---
assignees:
- claude-code
depends_on:
- 01KVTV9QTCZ47M8TVTTY28JR6G
- 01KVTVACR1W8HFKFR8DTAJPMP0
position_column: todo
position_ordinal: aa80
project: null
title: Consolidation — delete standalone read_file/glob_files/grep_files + validator registration
---
## What
The unified `files` ops subsume the standalone tools; delete the duplicates and a grammar special-case. Per the doc and the confirmed decision, **fully** remove the by-name split tools and point callers at the unified `files` op.

### Files to delete
- `crates/swissarmyhammer-tools/src/mcp/tools/files/read_file.rs`, `glob_files.rs`, `grep_files.rs` and the `pub mod` / `pub use` lines for them in `files/mod.rs`.
- `register_validator_file_tools` (in `files/mod.rs`) and its `test_register_validator_file_tools`.

### Both call sites of `register_validator_file_tools` must be repointed (NOT just the validator profile)
1. **Validator profile** — `tools::register_validator_tools` (`tools/mod.rs:84`): serve the unified `FilesTool::read_only()` so the validator surface stays read-only (no write/edit).
2. **Agent-tools server** — `McpServer::create_agent_tools_server` (`server.rs:1064-1090`, the registry llama-agent mounts in-process). It already calls `register_file_tools` (= unified `FilesTool::new()`, which keeps write/edit), then *additively* calls `register_validator_file_tools` for the by-name read-only forms. **Drop only the `register_validator_file_tools` call here** — llama keeps write/edit/read/glob/grep via the unified op-dispatched tool; it loses the by-name split forms (intended). Update the surrounding doc comments at `server.rs:1044-1052` and `1072-1073` that describe registering the split forms.

### Doc comments / stale references
- Update the `category()` doc comment in `files/mod.rs:142-148` (describes the split-tool validator surface).
- Update the validator-server doc comment at `server.rs:986-993`.
- Update the top-level `files/description.md` op summary to reflect the unified surface.
- Remove any grammar/schema special-case for the three split tool names.

### Tests to migrate (hard-coded by-name expectations — enumerate, don't rely on grep-clean alone)
These assert `read_file`/`glob_files`/`grep_files` by name and the "exactly 4 tools" validator count; migrate each to assert the unified `files` tool instead: `server.rs` lines ~2603, 2667, 2685, 2711, 2824-2862 (executes read/glob/grep by name on the validator server — convert to op-dispatched `files` calls), 3149-3161; and `tools/mod.rs:106`.

**Validator-surface caveat:** Hermes-trained validator models call tools **by name**. Switching them to op-dispatched `files` changes their advertised surface. Verification must EXECUTE an op-dispatched call, not just check registration. If a real validator/Hermes run regresses, log the gap and reference the diagnostics/validator project rather than leaving it silently broken.

## Acceptance Criteria
- [ ] `read_file.rs`/`glob_files.rs`/`grep_files.rs` and their `mod.rs` registrations are gone; the whole workspace builds (`cargo build`).
- [ ] `register_validator_file_tools` is removed; BOTH call sites repointed — validator profile serves `FilesTool::read_only()`, and `create_agent_tools_server` keeps the unified tool (write/edit retained) with the split call dropped.
- [ ] `rg 'ReadFileTool|GlobFilesTool|GrepFilesTool|register_validator_file_tools'` returns no hits; the by-name tests (server.rs 2603/2667/2685/2711/2824-2862/3149-3161, tools/mod.rs:106) are migrated, not just deleted.
- [ ] Stale doc comments (`files/mod.rs:142-148`, `server.rs:986-993`, `1044-1052`, `1072-1073`) and `files/description.md` updated.

## Tests
- [ ] Replace `test_register_validator_file_tools` with a test asserting the validator registry exposes the unified `files` tool (read-only) and not the three split names.
- [ ] Migrate the agent-tools-server and validator-server registry tests to the unified tool; the count/expected-name assertions reflect the new surface.
- [ ] A behavior integration test that **executes** an op-dispatched read AND glob AND grep through the validator-facing server (real output asserted), replacing the by-name execution test at server.rs:2824-2862.
- [ ] `cargo build` and `cargo test -p swissarmyhammer-tools` workspace-green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.