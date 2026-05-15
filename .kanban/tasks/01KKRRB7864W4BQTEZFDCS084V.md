---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffff9f80
title: Add PromptLibrary to ToolContext
---
## What

Add an `Option<Arc<RwLock<PromptLibrary>>>` field to `ToolContext` in `swissarmyhammer-tools/src/mcp/tool_registry.rs`. This makes the prompt rendering pipeline available to any tool that receives a `ToolContext`, not just skills and agents which hold their own copies.

**Files:**
- `swissarmyhammer-tools/src/mcp/tool_registry.rs` — add field to `ToolContext` struct + `ToolContext::new()`
- `swissarmyhammer-tools/src/mcp/server.rs` — wire prompt library into ToolContext at construction (lines ~739, ~1254)
- `swissarmyhammer-tools/src/test_utils.rs` — `create_test_context()` needs the field
- `swissarmyhammer-tools/tests/code_context_mcp_e2e_test.rs` — `make_context()`
- `swissarmyhammer-tools/tests/git_tool_integration_test.rs` — `make_test_context()`
- `swissarmyhammer-tools/tests/integration/file_size_limits.rs` — `create_test_context()`
- `swissarmyhammer-tools/tests/integration/file_rate_limiting.rs` — `create_test_context()`
- `swissarmyhammer-tools/tests/integration/file_tools_integrations.rs` — `create_test_context()`
- `swissarmyhammer-tools/tests/integration/web_search_integration.rs` — `create_test_context()`
- `swissarmyhammer-tools/tests/code_context_real_scenario_test.rs` — direct construction
- `swissarmyhammer-tools/src/mcp/tools/ralph/execute/mod.rs` — `make_context()`
- `swissarmyhammer-cli/src/mcp_integration.rs` — `CliToolContext` wraps ToolContext

**Blast radius:** ~12 construction sites. All use `ToolContext::new()` or struct literals. Adding a field to `new()` signature would break all of them, so instead: add the field with `None` default in `new()`, then set it with a builder method `with_prompt_library()` (same pattern as `plan_sender`).

## Acceptance Criteria
- [ ] `ToolContext` has a `pub prompt_library: Option<Arc<RwLock<PromptLibrary>>>` field
- [ ] `ToolContext::new()` initializes it to `None` (no signature change)
- [ ] Builder method `with_prompt_library()` exists
- [ ] Production construction in `server.rs` passes the real prompt library
- [ ] All 12+ construction sites compile without changes (they get `None` by default)

## Tests
- [ ] All existing tests pass unchanged: `cargo nextest run -p swissarmyhammer-tools`
- [ ] All CLI tests pass: `cargo nextest run -p swissarmyhammer-cli`
- [ ] No new test needed — this is additive plumbing with no behavior change