---
assignees:
- claude-code
position_column: todo
position_ordinal: '8280'
project: remove-prompts
title: Remove MCP prompt protocol surface (list_prompts/get_prompt) from the server
---
## What
Remove the rmcp MCP-protocol prompt endpoints that expose sah "prompts" to MCP clients. Keep the internal `PromptLibrary` (skills/agents render through it) — this task removes only the protocol-facing prompt capability, not the rendering engine.

Files to edit in `crates/swissarmyhammer-tools/src/mcp/server.rs`:
- Remove the rmcp trait impls `async fn list_prompts(...) -> ListPromptsResult` (~line 1893) and `async fn get_prompt(...) -> GetPromptResult` (~line 1940) and the related `list_prompt_*`/paginated handler around line 1977.
- Remove `caps.prompts = Some(PromptsCapability { ... })` (~line 220) so the server no longer advertises the prompts capability.
- Remove the public helper methods `McpServer::list_prompts` (~line 984) and `McpServer::get_prompt` (~line 1079) and the `load_all_prompts`-driven `initialize`/`reload` of *prompts for serving* (~line 944-971) IF they are only used to serve the MCP prompt protocol. NOTE: skill rendering still needs partials loaded into the library — verify via callgraph before deleting `load_all_prompts`; if it also loads `_partials`, keep partial loading and remove only the prompt-listing visibility logic.
- Remove the now-unused `is_prompt_visible` import and the prompt-visibility filtering. Coordinate with the common-crate cleanup task for deleting `is_prompt_visible` itself.

Use `get callgraph` (inbound) on `McpServer::list_prompts`, `McpServer::get_prompt`, and `load_all_prompts` before deleting to confirm no skill/agent path depends on them.

## Acceptance Criteria
- [ ] The MCP server no longer advertises `prompts` capability in its `initialize` response.
- [ ] No `list_prompts` / `get_prompt` rmcp handlers remain in `server.rs`.
- [ ] Skill and agent tools still render correctly (partials still load).
- [ ] `cargo build -p swissarmyhammer-tools` succeeds.

## Tests
- [ ] Add/update an MCP integration test in `crates/swissarmyhammer-tools/src/mcp/tests.rs` asserting the server `initialize` capabilities do NOT include `prompts`.
- [ ] Keep an existing skill-render integration test green (e.g. `tools/skill/use_op.rs` tests) to prove partial rendering survived.
- [ ] `cargo test -p swissarmyhammer-tools mcp::` is green.

## Workflow
- Use `/tdd` — write the "no prompts capability" assertion first; run callgraph checks before each deletion.