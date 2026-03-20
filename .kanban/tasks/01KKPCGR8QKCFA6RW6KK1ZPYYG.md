---
position_column: done
position_ordinal: ffffffffff9e80
title: Forward-compat HookEvent::kind() maps to Notification — misleading
---
`hookable_agent.rs` — HookEvent::kind()

The 6 forward-compat HookEvent variants (Elicitation, ElicitationResult, InstructionsLoaded, ConfigChange, WorktreeCreate, WorktreeRemove) all return `HookEventKind::Notification` from `kind()`. This is misleading — if someone registers a Notification hook, these events would incorrectly match it and fire the hook.

These events should either:
1. Get their own HookEventKind variants (preferred — they already exist in HookEventKindConfig)
2. Return a new `HookEventKind::Unknown` or similar sentinel that no config can match

The current mapping could cause unintended hook execution if a Notification hook is registered and one of these events is manually fired.

**Severity**: warning