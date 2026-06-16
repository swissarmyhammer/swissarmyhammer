---
assignees:
- claude-code
position_column: todo
position_ordinal: fb80
title: 'Bug: emit_view_switch double-definition in swissarmyhammer-kanban/src/scope_commands.rs:18'
---
## What

The review engine's full-tree sweep flagged a genuine blocker: a double-definition of `emit_view_switch` in `crates/swissarmyhammer-kanban/src/scope_commands.rs:18`.

Discovered incidentally during the review of z3ax1jz (a UI-test card); out of scope there, captured here so it isn't lost.

## Acceptance Criteria
- [ ] Confirm the double-definition at `crates/swissarmyhammer-kanban/src/scope_commands.rs:18`
- [ ] Resolve it (remove/merge the duplicate) so there is a single source of truth
- [ ] Workspace builds clean; relevant tests green

#bug