---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffd280
project: agent-builtins
title: 'tools: ToolCategory metadata; delete is_agent_tool/remove_agent_tools/agent_mode gating'
---
Replace the per-tool agent-only boolean + post-hoc subtraction with structural category metadata. Composition, not subtraction.

## Change
- Add a `category()` to the `McpTool` trait (`crates/swissarmyhammer-tools/src/mcp/tool_registry.rs`) returning one of: `Shared`, `Agent`, `Replacement { native: &str }`. (A tool may be Agent *and* Replacement — model Replacement as Agent + a `replaces` native-tool name; shell is the only one today, replacing `Bash`.)
- Assign categories:
  - **Agent**: `ReadFile`, `Files` (write/edit), `GlobFiles`, `GrepFiles`, `Web`, `Skill`, `Agent` (subagent delegation)
  - **Replacement (Agent + replaces Bash)**: `Shell`
  - **Shared**: `Ralph`, `Kanban`, `Code Context`, `Git`, `Question`
- Delete `is_agent_tool()` (`tool_registry.rs:922`), `AgentTool` marker trait, `remove_agent_tools()` (`tool_registry.rs:1116`) and its call in `crates/swissarmyhammer-tools/src/mcp/server.rs:654`.
- Delete the `agent_mode = executor != ClaudeCode` registry gating in `apps/swissarmyhammer-cli/src/mcp_integration.rs:135` and thread-through (`unified_server.rs`, `new_with_agent_mode`). Host-conditional behavior moves to the serve boundary (see per-client composition + Bash-deny cards).
- Scrub stale "for llama-agent / Claude Code" comments in `files/*/mod.rs`, `shared_utils.rs`, `unified_server.rs`.

## Notes
- The phantom `llama-agent`/`claude-agent` deps were already removed from `tools/Cargo.toml` (build-green); this card does not reintroduce them.
- `tools.yaml` enable/disable (tool_config.rs) is orthogonal and stays.

## Done when
- `cargo build -p swissarmyhammer-tools` green; no `is_agent_tool`/`remove_agent_tools`/`agent_mode` references remain in tools.
- Every tool reports a category.

## Review Findings (2026-06-03 06:52)

Scope: uncommitted working-tree diff vs HEAD (the `git get changes` parent-branch detection surfaced unrelated committed worktree files — skills, workspace-init, code-context-cli — which are NOT part of this card and were excluded). Effective scope is the 31 modified files implementing the ToolCategory change.

Verified clean: trait + enum change is well-documented; deletions are complete (`is_agent_tool`, `AgentTool` marker trait, `remove_agent_tools`, and the `agent_mode` parameter are fully removed — no orphaned references; remaining `agent_mode` hits are the unrelated `claude-agent` crate's own config field and a test-fn name). Every tool's `category()` matches the intended taxonomy (Agent: web/files-all-variants/skill/agent/glob/grep; Replacement{native:"Bash"}: shell; Shared default: ralph/kanban/code_context/git/question). The `Doctorable::category` vs `McpTool::category` disambiguation is handled correctly in all 4 health-check impls (agent, code_context, shell, web) via fully-qualified calls — no accidental shadowing. Downstream callers (avp-common, kanban-app, llama-agent test, mcp-proxy, unified_server) updated correctly, not papered over; dead `agent_mode_for_validator()` helper + its 2 tests removed, unused `ModelExecutorType` import dropped. `cargo build -p swissarmyhammer-tools -p avp-common` green; `cargo test --lib category` → 20 passed. Per the card, ToolCategory having no inbound consumer yet and the full union being served to every client are intentional intermediate states — not flagged.

### Warnings
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:518` — No test asserts the `ShellExecuteTool` actually returns `ToolCategory::Replacement { native: "Bash" }`. The only Replacement test (`tool_registry.rs:test_replacement_category_carries_native`) checks enum-literal equality (`Replacement{native:"Bash"} == Replacement{native:"Bash"}`), exercising the derived `PartialEq`, not the shell tool's override. This is the single most semantically significant assignment (unique variant + payload) and the one most likely to regress; it should have a direct `assert_eq!(McpTool::category(&ShellExecuteTool::new()), ToolCategory::Replacement { native: "Bash" })`. Suggestion: add a one-line category test in shell/mod.rs's test module mirroring the ralph/files pattern.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/web/mod.rs:63`, `skill/mod.rs:147`, `agent/mod.rs:162`, `files/glob_files.rs:86`, `files/grep_files.rs:102` — Agent-category overrides on web, skill, agent (subagent), glob_files, and grep_files have no per-tool test asserting `McpTool::category() == ToolCategory::Agent`. Tested today: ralph (Shared), files-all + files-read-only + read_file (Agent). The card asks "tests asserting each tool's category"; coverage is partial. Since the taxonomy is exactly the kind of parallel-assignment data a human must keep in lockstep, a per-tool assertion guards each one against silent drift. Suggestion: add the same one-line assertion to each tool's existing test module (cheap, follows the established pattern).

### Nits
- [x] `crates/swissarmyhammer-tools/src/mcp/tool_registry.rs:3152` — `test_replacement_category_carries_native` only proves the derived `PartialEq`/`Ne` on enum literals; its name implies it verifies a tool "carries" the native name but no tool is involved. Once a real shell-tool category test exists (warning above), consider renaming this to reflect that it covers enum equality semantics, or fold the coverage into the shell test.