---
assignees:
- wballard
position_column: done
position_ordinal: ffffffffffffffffffffffff8780
title: Verify llama-agent fetches tool schemas from validator MCP server (not just injected ToolDefinition)
---
**Critical end-to-end gap.** The Qwen3 strategy work (`01KQ35KFJXJ70GNB4ZPRJD6R43`) was verified by `llama-agent/tests/integration/tool_call_round_trip.rs`, which **directly injects** a `ToolDefinition` onto the session: `session.available_tools = vec![read_file_tool()];`. It never tested the MCP path — *llama-agent connects to the configured `McpServerConfig` URL, calls `tools/list`, populates `Session.available_tools`, renders them via `format_tools_for_qwen3`, sends the prompt*.

That whole path is unverified. If it's broken (or only works for the `agent_mode: true` bundle that already had MCP integration), the always-on validator MCP server (`01KQ35MHFJQPMEKQ08PZKBKFY0`) will be running and bound, but the validator session will still see `available_tools: vec![]` and we're back to the morning failure mode through a different door.

## Verdict (2026-04-27)

**Path existed but was broken.** Outcome 2 from the task description: "Path exists but is gated/broken — find the gate, fix it, prove it."

### What was wrong

Two production code paths called `MCPClient::list_tools()`, which intentionally returned `Vec<String>` (just tool names), throwing away rmcp `Tool.description` and `Tool.input_schema`:

1. **`AgentServer::discover_tools`** — `llama-agent/src/agent.rs:1238` (manual fetch path used by `AgentAPI::discover_tools`)
2. **`AcpServer::new_session`** — `llama-agent/src/acp/server.rs:1180-1199` (auto-fetch path that fires when ACP `default_mcp_servers` is non-empty — i.e. the validator path)

Both call sites then constructed `ToolDefinition` with placeholder values:
- `description: format!("Tool: {}", name)` (a string template, not the real description)
- `parameters: serde_json::Value::Object(serde_json::Map::new())` (an empty schema)

These get fed straight into `format_tools_for_qwen3` (`llama-agent/src/chat_template.rs:769`), which serialises `tool.parameters` byte-for-byte into the `# Tools` block of the system prompt. With an empty `{}` for `parameters`, the model has no way to know what arguments any tool accepts. Even if the validator MCP server was bound and reachable, every validator session was rendering useless tool schemas.

### What was fixed

- Added `tool_to_definition()` helper in `llama-agent/src/mcp.rs` that converts an rmcp `Tool` (preserving `description` and `input_schema`) into a llama-agent `ToolDefinition`.
- Added `UnifiedMCPClient::list_tools_with_schemas()` returning `Vec<ToolDefinition>` — same transport call as `list_tools()` but preserves the schema instead of dropping it.
- Added `MCPClient::list_tools_with_schemas` to the trait with a default implementation that falls back to `list_tools` + placeholder schema (preserves backward compat for mock impls used in `acp_slash_command.rs`).
- Switched `AgentServer::discover_tools` and `AcpServer::new_session` to use the new method. The placeholder description / empty-schema construction is gone from production paths.

### Test verification

Two new integration tests in `llama-agent/tests/integration/tool_call_round_trip_via_mcp.rs`:

1. **`test_discover_tools_via_validator_mcp_server_fetches_real_schemas`** — fast (no model needed beyond model loader init). Spins up a real `start_mcp_server_with_options` validator server in HTTP mode, builds an `AgentServer` with a single `MCPServerConfig::Http` pointing at `/mcp/validator`, calls `discover_tools`, and asserts:
   - `Session.available_tools` is non-empty
   - At least one tool carries a real (non-placeholder) description AND a JSON Schema with a `type` field
   - `read_file` is present and its schema includes a `path`-like property

   **Result**: Discovered 4 tools (`code_context`, `read_file`, `glob_files`, `grep_files`) all with real schemas (`type`, `properties`, `required`, etc.) — the fetch path is now healthy.

2. **`test_full_round_trip_with_mcp_fetched_tools_against_real_model`** — full chain. Same setup, then prompts Qwen3-0.6B to use a tool, asserts the model emitted at least one parseable tool call whose `name` matches a tool from the fetched list.

   **Result**: Model reasoned over the schema (`"The read_file tool's parameters include path, which is required"`) and emitted a valid `<tool_call>{"name":"read_file","arguments":{"path":"/tmp/example.rs"}}</tool_call>`. Proves the fetch path delivers schemas the model can actually act on.

### Coordination with parallel agent

`01KQ7M39FN2EC3GV9414MNF3MD` (multi-turn tool-use loop) lives in `tool_use_multi_turn.rs` and uses its own minimal `read_file_mcp_server.rs` fixture (in-process http server with a single `read_file` tool). That agent owns the dispatch loop verification. This agent owns the fetch verification. Different files, different fixtures, no overlap on the changes inside `mcp.rs`/`agent.rs`/`acp/server.rs`.

## What this card proves

- **`mcp_servers: 1`** for every validator session: passing a non-empty `mcp_servers` list to `AgentServer::initialize` now produces a real schema-carrying `Session.available_tools` after `discover_tools` runs against the validator MCP server.
- **Real schemas in the chat template**: the fetched `parameters` JSON is the same JSON the validator server publishes via `tools/list`, so `format_tools_for_qwen3` renders the canonical Qwen3 `# Tools` block from real schema data, not from `{}`.
- **Round-trip works under a real model**: Qwen3-0.6B loads the tool definitions, reasons about the schema, and emits a parseable tool call against the fetched tool list. Tool dispatch is tested separately by the sister card.

## Pairs with

- `01KQ35MHFJQPMEKQ08PZKBKFY0` (always-on validator MCP server). This card verifies that the URL the tools task produces actually flows through to the chat template.
- `01KQ7KYNBEHQGEMGND4AEG9EV6` (tool name mismatch). After this card lands and the MCP fetch is proven, that card's split tools get verified end-to-end. #llama-agent