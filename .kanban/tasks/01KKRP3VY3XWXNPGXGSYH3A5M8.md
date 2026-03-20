---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffca80
title: 'avp-common/strategy/claude/strategy.rs: new hook types all route to Chain::success() with no validator support'
---
avp-common/src/strategy/claude/strategy.rs:431-498

All 9 new hook types (Elicitation, ElicitationResult, InstructionsLoaded, ConfigChange, WorktreeCreate, WorktreeRemove, PostCompact, TeammateIdle, TaskCompleted) are wired to `Chain::success()`. This means no validators can be attached to them — user-defined validators will be silently ignored for these events.

The pattern `Chain::success().execute(&typed)` appears to be a placeholder for "not yet implemented". Several of these types have corresponding output types with `block`/`allow`/`deny` constructors (e.g., `AvpWorktreeCreateOutput::deny`, `AvpTeammateIdleOutput::block`), which implies validators *should* be able to act on them.

If this is intentional (e.g., validator chains will be added later), add a TODO comment per hook type explaining what chain should eventually be used and whether validators should have access. If unintentional, this is a functionality gap where users configure validators for these events and they silently do nothing.