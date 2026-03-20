---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffcd80
title: 'avp-common/types/avp_output.rs: massive code duplication across new output types'
---
avp-common/src/types/avp_output.rs (new types section)

The 7 new blockable output types (`AvpElicitationOutput`, `AvpElicitationResultOutput`, `AvpConfigChangeOutput`, `AvpWorktreeCreateOutput`, `AvpTeammateIdleOutput`, `AvpTaskCompletedOutput`, plus the 3 observe-only types) each implement nearly identical `allow()`, `block()`/`deny()`, and `block_from_validator()`/`deny_from_validator()` methods. The bodies are copy-pasted with only field name differences (`allow`, `allow_idle`, `block_reason`, `deny_reason`).

This is a maintenance hazard — a bug fix in one `deny_from_validator()` (e.g., changing `should_continue: true`) must be applied to all 6-7 copies manually.

Suggestion: introduce a macro (e.g., `impl_blockable_output!`) or a shared `BlockableOutput<T>` generic wrapper that delegates to a common implementation. The observe-only types could also use a macro or a single unit struct wrapping `AvpOutputBase`. #review-finding