---
assignees:
- claude-code
position_column: todo
position_ordinal: '8280'
project: claude-hooks
title: Carry the bare tool name to hook events for matcher fidelity
---
Hook tool-name matchers (PreToolUse/PostToolUse/PostToolUseFailure) must test against the BARE tool name (e.g. `fs_read`, `shell`, `mcp__server__tool`). Today `notification_to_events` in hookable_agent.rs uses `tool_call.title`, and `crates/llama-agent/src/acp/translation.rs` sets ACP `ToolCall.title` to the tool name OR `"<name>: <description>"` (see tests asserting `title == "fs_read: Read a file from disk"`). So matchers would test against a human title with a description suffix and never match cleanly.

## Scope
- Thread the bare tool name into hook events. Options: (a) when the real tool-dispatch seam fires PreToolUse/PostToolUse (see the real-seam task), pass the llama tool's actual name directly — preferred, since that seam has the un-decorated name; (b) for any notification-derived events that remain, carry the bare name via the tool-call record / a meta field rather than `title`.
- Ensure `HookEvent::PreToolUse.tool_name` etc. hold the bare name everywhere they are constructed.
- Document the inherent name-mapping divergence: Claude's tool names (`Bash`, `Edit`, `Write`) differ from llama-agent's (`shell`, `fs_write`, `fs_read`, MCP tools as `mcp__<server>__<tool>`). Users must write matchers against llama-agent's names. Capture the canonical name list in the docs task.

## Acceptance criteria + tests
- A PreToolUse hook with `matcher: "fs_read"` fires for the fs_read tool even when the ACP title is `"fs_read: Read a file from disk"`.
- An MCP tool surfaces as `mcp__<server>__<tool>` so `mcp__.*` regex matchers work.
- No event carries a description-suffixed name into matcher evaluation.