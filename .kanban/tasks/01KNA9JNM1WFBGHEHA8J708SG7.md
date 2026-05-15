---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffec80
title: Fix context menu command ordering — innermost scope commands should appear first
---
## What

When right-clicking an attachment (or any scoped item), the context menu shows task commands before attachment commands. The innermost scope's commands should appear first.

**Root cause** in `swissarmyhammer-kanban/src/scope_commands.rs` `commands_for_scope()`:

1. **Step 1** (lines 190–238) walks the scope chain and adds entity schema commands (from entity YAML definitions). These come first in the result vec. But `attachment` has no entity definition — it's not in the entity YAML schema. So attachment commands don't appear here at all.

2. **Step 2** (lines 242–288) adds registry commands (from command YAML like `attachment.yaml`). `attachment.open` and `attachment.reveal` land here with `group: "global"`. They appear AFTER all entity commands from step 1.

The result: task entity commands (Copy, Cut, Inspect, Archive) appear first, then attachment commands (Open, Show in Finder) appear at the bottom mixed with global commands.

**Fix**: In step 2, when a registry command has a `scope` that matches a moniker in the scope chain, set its `group` to the matching scope type (e.g. `"attachment"`) instead of `"global"`. Then sort the final result so innermost scope groups appear first — matching the scope chain order.

Alternatively, interleave step 1 and step 2 by scope chain position: for each moniker in the scope chain, first emit entity schema commands, then emit registry commands scoped to that type, then move to the next moniker.

**Files to modify**:
- `swissarmyhammer-kanban/src/scope_commands.rs` — `commands_for_scope()` ordering logic

## Acceptance Criteria
- [ ] Right-clicking an attachment shows Open and Show in Finder before task commands
- [ ] Right-clicking a tag pill still shows tag commands (Copy Tag, Inspect Tag) before task commands
- [ ] Global commands (Undo, Redo) remain at the end
- [ ] Group separators still appear between different scope levels

## Tests
- [ ] Add test in `scope_commands.rs`: scope chain `["attachment:/path", "task:01X", "column:todo"]` → first commands are `attachment.open` and `attachment.reveal`, then entity commands
- [ ] Existing scope_commands tests still pass
- [ ] `cargo nextest run -p swissarmyhammer-kanban` — all pass (except pre-existing attachment failures)