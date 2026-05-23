---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
title: 'Kanban agent: auto-approve all tool permissions (no per-tool nag)'
---
## What

The kanban AI agent prompts the user for permission on tool calls (the inline `PermissionPrompt` in `apps/kanban-app/ui/src/components/ai-panel.tsx`). The user wants the kanban agent to **auto-approve all tool calls by default** — no per-tool nag, including the Claude CLI's built-in tools (Write / Edit / Bash / etc.), not just the board's MCP tools.

### Root cause (researched)

- The Claude CLI is already spawned with `--dangerously-skip-permissions` (`crates/claude-agent/src/claude_process.rs:152`), so the CLI does not prompt internally. The sole remaining gate is **claude-agent's own `PermissionPolicyEngine`**.
- claude-agent's `handle_tool_permission_check` (`crates/claude-agent/src/agent_prompt_handling.rs:894`) runs `evaluate_tool_call`; on `RequireUserConsent` it calls `request_user_permission`, which sends ACP `session/request_permission` to the kanban UI → the nag.
- The kanban agent is built by `swissarmyhammer-agent::create_claude_agent` (`crates/swissarmyhammer-agent/src/lib.rs:754`), which sets `auto_allow_tool_patterns = vec![MCP_AUTO_ALLOW_PATTERN]` (`"mcp__*"`). That auto-approves the board's MCP tools but leaves everything else (the CLI built-ins) to fall through to `default_permission_policies()`'s catch-all `"*"` → `AskUser` (`crates/claude-agent/src/permissions.rs:580-621`). Those are the prompts the user sees.
- `create_permission_engine` (`crates/claude-agent/src/agent.rs:232`) turns each `auto_allow_tool_patterns` entry into a `PolicyAction::Allow` policy prepended before the defaults; matching is first-match-wins, so a `"*"` Allow entry auto-approves every tool before any `AskUser` policy is reached → no `session/request_permission` is ever emitted.

### Approach (root-cause, agent-level — the agent simply never asks)

1. `crates/swissarmyhammer-agent/src/lib.rs`: add `auto_allow_all: bool` to `CreateAgentOptions` (struct at ~line 300). Extract a small pure helper, e.g. `fn resolve_auto_allow_patterns(auto_allow_all: bool) -> Vec<String>` returning `vec!["*".to_string()]` when true and `vec![MCP_AUTO_ALLOW_PATTERN.to_string()]` (the current `"mcp__*"`) when false. Thread the flag through `create_agent_with_options` → `create_claude_agent` (add a param) and use the helper to set `auto_allow_tool_patterns` in both `AgentConfig` branches (the `Some(mcp)` and `None` branches at ~756-776). Default (`auto_allow_all: false`) keeps existing behavior for all other callers (e.g. validator agents).
2. `apps/kanban-app/src/ai/agent_ws.rs:163`: change `create_agent(&model_config, None)` to `create_agent_with_options(&model_config, None, CreateAgentOptions { auto_allow_all: true, ..Default::default() })` (import `create_agent_with_options` + `CreateAgentOptions`). This is the kanban app's only agent-creation site (`handle_connection`).

Net effect: the kanban agent's policy engine returns `Allowed` for every tool, so `request_user_permission` never fires and the UI never receives a `session/request_permission` — auto-approve-all, no nag, on by default.

### Deliberate decision / safety note

This auto-approves ALL tools (including Bash/terminal/network), matching the user's explicit request and consistent with the existing posture (the CLI already runs `--dangerously-skip-permissions`). It is scoped to the kanban app via the new option — other `create_agent` consumers are unchanged.

### Non-goals

- Do NOT change `default_permission_policies()` or the global default of `auto_allow_tool_patterns` (keep other consumers' behavior intact).
- Do NOT remove the UI `PermissionPrompt` / `respondPermission` machinery — it stays for any agent that still asks; the kanban agent simply won't trigger it.
- A user-facing on/off toggle is out of scope (always-on for the kanban agent). If wanted, file a follow-up.

## Acceptance Criteria

- [ ] `CreateAgentOptions` has an `auto_allow_all` field; when `true`, the Claude `AgentConfig` is built with `auto_allow_tool_patterns == ["*"]`; when `false`, it is `["mcp__*"]` (unchanged default).
- [ ] A `ClaudeAgent` built with `auto_allow_tool_patterns: ["*"]` evaluates a non-MCP built-in tool (e.g. `fs_write_file` / `terminal_create`) to an auto-allowed outcome — NOT `RequireUserConsent`.
- [ ] The kanban app's `agent_ws::handle_connection` creates the agent with `auto_allow_all: true`.
- [ ] Other `create_agent` callers (default options) are unaffected: `auto_allow_tool_patterns` stays `["mcp__*"]`.

## Tests

- [ ] `crates/swissarmyhammer-agent/src/lib.rs`: unit test for `resolve_auto_allow_patterns` — `true → ["*"]`, `false → ["mcp__*"]` (mirrors the existing config test near line 1579). Run `cargo test -p swissarmyhammer-agent resolve_auto_allow` → green.
- [ ] `crates/claude-agent/src/agent.rs`: add a sibling to `test_auto_allow_tool_patterns_skip_consent_for_mcp_tools` (~line 3014) that builds a `ClaudeAgent` with `auto_allow_tool_patterns: vec!["*".to_string()]` and asserts a non-MCP built-in tool (e.g. `terminal_create`) is auto-allowed (no `RequireUserConsent`). Run `cargo test -p claude-agent auto_allow` → green.
- [ ] Build/typecheck the kanban app wiring: `cargo check -p kanban-app` (or the app's package name) green with the `create_agent_with_options` call.
- [ ] `cargo test -p swissarmyhammer-agent -p claude-agent` green.

## Workflow

- Use `/tdd` — write the `resolve_auto_allow_patterns` test and the claude-agent `"*"`-auto-allow eval test first (red), then implement the option + wiring to green.

#bug