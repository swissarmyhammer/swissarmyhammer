---
position_column: done
position_ordinal: ffffffffffa680
title: AvpWorktreeCreateOutput missing deny_from_validator constructor
---
`avp-common/src/types/avp_output.rs` — AvpWorktreeCreateOutput

AvpWorktreeCreateOutput has `deny()` but no `deny_from_validator()`. The existing deny-capable type AvpPreToolUseOutput has `deny_from_validator()`. AvpWorktreeCreateOutput should follow the same pattern.

**Severity**: nit