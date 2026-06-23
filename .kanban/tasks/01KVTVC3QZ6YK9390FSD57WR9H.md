---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvvcp6w0jdqjcfe0m016fj82
  text: |-
    Picked up. Both deps (y28jr6g read-hashline, tajpmp0 edit-cascade) are done. Research complete.

    Key findings vs task description:
    - description.md is ALREADY unified (no split-tool refs) — nothing to change there.
    - Found an extra by-name test NOT enumerated in the task: crates/llama-agent/tests/integration/agent_tools_mount.rs:66-73 asserts the agent-tools mount serves read_file/glob_files/grep_files by name. Since create_agent_tools_server drops register_validator_file_tools, that second loop must be migrated (llama keeps files via unified op; loses by-name forms — intended).
    - read/mod.rs:7,144 and glob_files.rs:8 / grep_files.rs:8 doc comments reference ReadFileTool — must clean up so rg returns zero.
    - No grammar/schema special-case for the three names found in code (only kanban .md task files).
    Starting TDD.
  timestamp: 2026-06-23T23:22:46.528245+00:00
- actor: claude-code
  id: 01kvvdpfb7hvvs8ywqg1r47s0q
  text: |-
    Implementation landed (TDD: tests migrated first, watched the old by-name assertions fail, then implemented).

    DELETED:
    - crates/swissarmyhammer-tools/src/mcp/tools/files/{read_file.rs,glob_files.rs,grep_files.rs}
    - pub mod glob_files/grep_files/read_file + pub use {GlobFilesTool,GrepFilesTool,ReadFileTool} in files/mod.rs
    - register_validator_file_tools + test_register_validator_file_tools (replaced with test_register_validator_files_tool_is_unified_read_only)

    CALL SITE 1 (validator profile, tools/mod.rs): register_validator_tools now does registry.register(files::FilesTool::read_only()). Doc comment updated.
    CALL SITE 2 (server.rs create_agent_tools_server): dropped the register_validator_file_tools call + its local `use`; keeps register_file_tools (unified FilesTool::new(), write/edit retained). Doc comments at the validator-server doc, agent-tools-server doc, and the verbatim-serve comment updated.

    DOC UPDATES: files/mod.rs category() doc; read/mod.rs module + execute_read docs (dropped ReadFileTool refs); tools/mod.rs register_validator_tools profile doc. description.md was ALREADY unified (no change needed).

    TESTS MIGRATED (by-name -> unified files / op-dispatched):
    - tools/mod.rs: test_validator_profile_membership expected set -> {code_context, files}
    - server.rs: test_validator_server_has_only_validator_tools (4->2 tools, files present, split absent); test_validator_context_registry_is_isolated (same); test_validator_server_serves_exactly_the_profile (expected {code_context,files}); test_create_validator_server_tool_execution (op-dispatched read)
    - server.rs NEW execution test: test_validator_files_tool_is_read_only_and_op_dispatched — EXECUTES op-dispatched read+glob+grep through the validator-facing server and asserts real output (hello world / test.txt / hello), asserts write+edit ops rejected + file unchanged on disk, asserts split by-name names are Unknown. Replaces the old by-name execution test.
    - unified_server.rs: test_validator_endpoint_lists_only_validator_tools (HTTP /mcp/validator e2e) expected {files,code_context}; split names added to forbidden list.
    - llama-agent agent_tools_mount.rs: second loop now asserts split names are ABSENT.
    - llama-agent tool_call_round_trip_via_mcp.rs (NOT in task enumeration, found via double-check): both tests migrated read_file anchor -> unified files (op property), + negative split-tool assertions; real-model round-trip now instructs op "read file" and the model emits a `files` call.

    VERIFIED: cargo nextest run -p swissarmyhammer-tools = 1358 passed 0 failed; doctests --doc 8 passed; llama-agent agent_tools_mount + tool_call_round_trip_via_mcp (incl. real Qwen3 model) green; cargo fmt --check clean; cargo clippy -p swissarmyhammer-tools --tests -D warnings clean. rg 'ReadFileTool|GlobFilesTool|GrepFilesTool|register_validator_file_tools' crates/ = ZERO hits.

    Out of scope confirmed: llama-agent acp_agentic_loop.rs / acp_hooks_real_model.rs / read_file_mcp_server.rs use a self-contained ReadFileMcpServer fixture (hand-built `read_file` tool), NOT the SAH validator surface — independent of the deleted production tool. The ideas/file-edit-tools.md research doc mentions ReadFileTool only as the "before" state being removed (narrative, not code).
  timestamp: 2026-06-23T23:40:23.783228+00:00
depends_on:
- 01KVTV9QTCZ47M8TVTTY28JR6G
- 01KVTVACR1W8HFKFR8DTAJPMP0
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffdc80
project: file-edit-tools
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