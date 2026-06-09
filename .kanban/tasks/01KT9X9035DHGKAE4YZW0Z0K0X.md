---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffe980
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

## Review Findings (2026-06-04 18:56)

Acceptance criteria all met and exercised by passing tests: matcher `fs_read` fires despite a decorated title (`test_pre_tool_use_matcher_fires_on_bare_name_despite_decorated_title`); MCP tools surface as `mcp__<server>__<tool>` (`test_pre_tool_use_mcp_tool_surfaces_qualified_name`, `test_tool_call_to_acp_meta_bare_name_is_mcp_qualified`); the cached name reused by Post events is bare (`test_notification_to_events_pre_tool_use_uses_bare_name_from_meta`); fallback to title when meta absent preserves prior behaviour (`test_notification_to_events_falls_back_to_title_without_meta`). The shared `TOOL_NAME_META_KEY` constant is reused across the crate boundary rather than re-stringified; `bare_tool_name` and the meta write are correctly extracted. Verified: `agent-client-protocol-extras` + `llama-agent` test suites pass (218/67/1/1 + 1032 lib), clippy clean on both with `--all-targets`.

### Warnings
- [x] `crates/agent-client-protocol-extras/src/hookable_agent.rs` (the `///` block immediately above `fn bare_tool_name`) — Inserting `bare_tool_name` between `notification_to_events` and its existing doc comment left the docs misattached. The block now reads as `notification_to_events`'s doc ("Convert a `SessionNotification` into the hook events…", "Tracks tool-call ids in `tool_names`…") followed without separation by `bare_tool_name`'s own doc ("Extract the bare tool name a matcher should test against…"). Net effect: `bare_tool_name` carries two leading paragraphs describing a different function, and `notification_to_events` (the `fn notification_to_events` below) is now left with no doc comment at all. Move the `notification_to_events` paragraphs back above `fn notification_to_events`, leaving only the "Extract the bare tool name…" paragraph as `bare_tool_name`'s docstring. FIXED: split the doc block — the `notification_to_events` paragraphs now sit above `fn notification_to_events`, and `bare_tool_name` keeps only its own "Extract the bare tool name…" paragraph. Verified: `agent-client-protocol-extras` tests pass (67/1/1, lib included), clippy `--all-targets` clean.