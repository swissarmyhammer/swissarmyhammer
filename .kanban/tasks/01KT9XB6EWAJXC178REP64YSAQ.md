---
assignees:
- claude-code
depends_on:
- 01KT9XAK5FB9Y91XBBZDMFMHW9
- 01KT9X9035DHGKAE4YZW0Z0K0X
position_column: todo
position_ordinal: '8680'
project: claude-hooks
title: Fire PreToolUse/PostToolUse/PostToolUseFailure at the real tool-dispatch seam (true blocking)
---
Make PreToolUse hooks truly DENY a tool call (Claude Code semantics), not just cancel the turn. The agentic loop dispatches tools at `crates/llama-agent/src/acp/server.rs:2400` via `super::translation::handle_tool_call(...)`. Fire tool hooks synchronously around that call instead of from the notification stream.

## Scope
At the tool-dispatch seam, for each tool call the model emits:
1. Build a `HookEvent::PreToolUse { session_id, tool_name (bare name), tool_input, tool_use_id, cwd }` and fire matching hooks (use the session's HookableAgent / registrations from the wiring task). Evaluate decisions:
   - `Block`/permissionDecision `deny` → DO NOT execute the tool. Synthesize a tool result carrying the deny reason and feed it back to the model as the tool's output (so the loop continues with the model informed), matching Claude's "blocked" behavior. Emit the appropriate ACP ToolCallUpdate (failed/denied).
   - `AllowWithUpdatedInput`/`updatedInput` → replace the tool arguments with the updated input BEFORE calling `handle_tool_call` (now genuinely possible at this seam — unlike the notification path).
   - `AllowWithContext`/additionalContext → inject the context back to the model alongside the tool result.
   - `Allow` → proceed normally.
2. After execution, fire `PostToolUse` (success) or `PostToolUseFailure` (error). Feed `additionalContext` / exit-2 stderr back to the model as Claude does. These cannot block (action already happened).
3. Run multiple matching hooks; first Block wins (mirror existing decision-priority helpers). Honor `continue:false` → stop the turn.

## Coordinate with existing code
- Remove/redirect PreToolUse/PostToolUse/PostToolUseFailure emission from `intercept_notifications` (hookable_agent.rs) so events don't fire twice — leave that path for `Notification`-family only (per the wiring task). The notifications are still BROADCAST for the client UI; only the hook-firing moves.

## Acceptance criteria + tests (scripted model that emits a tool call)
- A PreToolUse hook with matcher for the tool and a `deny` decision prevents `handle_tool_call` from running; the model receives the deny reason as the tool result and the turn continues.
- A PreToolUse `updatedInput` rewrites the tool arguments actually passed to `handle_tool_call`.
- PostToolUse additionalContext is delivered to the model after a successful call; PostToolUseFailure fires on a failed call.
- No double-firing: a single tool call fires PreToolUse exactly once.