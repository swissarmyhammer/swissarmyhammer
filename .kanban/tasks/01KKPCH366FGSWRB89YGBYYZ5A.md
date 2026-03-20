---
position_column: done
position_ordinal: ffffffffff9380
title: strategy.rs uses NotificationInput as stand-in — loses event-specific fields
---
`avp-common/src/strategy/claude/strategy.rs:429-441`

The forward-compat match arm deserializes all 9 new hook types as `NotificationInput`. This works for pass-through (Chain::success()), but:

1. Event-specific fields (mcp_server_name, worktree_path, etc.) are silently discarded
2. The proper input types already exist (ElicitationInput, ConfigChangeInput, etc.) — they should be used even for pass-through chains, so that if chain logic is added later, the typed data is available
3. Each new type should have its own match arm using its dedicated input type, following the pattern of the other pass-through hooks

This is a correctness issue when validators are eventually added for these hook types — they'd receive a NotificationInput instead of the correct typed input.

**Severity**: warning