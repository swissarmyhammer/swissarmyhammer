---
assignees:
- claude-code
position_column: todo
position_ordinal: '8180'
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