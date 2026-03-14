---
position_column: done
position_ordinal: z00
title: Wire TeammateIdle/TaskCompleted/PostCompact via ext_notification
---
## What
HookableAgent passes `ext_notification()` through without inspection. If the inner agent sends extension notifications for teammate idle, task completion, or post-compaction events, these could be intercepted to fire the corresponding hooks.

Investigate:
1. Does Claude Code or any inner agent send ext_notifications for these events?
2. If so, what method/notification names are used?
3. If not, these hooks may need a different firing mechanism (e.g., a public `fire_event()` method on HookableAgent that callers can use)

Alternative approach: add a `pub async fn fire_event(&self, event: HookEvent) -> Vec<HookDecision>` method to HookableAgent so callers (like the CLI or MCP proxy) can fire arbitrary hook events without going through the Agent trait.

## Acceptance Criteria
- [ ] Determine the right firing mechanism for each hook type
- [ ] Implement firing for at least one of: TeammateIdle, TaskCompleted, PostCompact
- [ ] E2e test proving the hook fires and decisions are returned

## Tests
- [ ] Test that fire_event (or equivalent) runs registered hooks
- [ ] Test that matcher filtering works for fired events
- [ ] Run `cargo test -p agent-client-protocol-extras` — all pass