---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffd580
title: '[WARNING] UseSkill.arguments field is decoded but never passed to execute()'
---
## What

In `swissarmyhammer-tools/src/mcp/tools/skill/use_op.rs`, `execute_use()` extracts `skill_arguments` from the incoming JSON map and passes it to `render_skill_instructions()`. However, `UseSkill::new(name)` is constructed without calling `.with_arguments()`, so `UseSkill.arguments` is always `None` inside `execute()`.

The field is added to the struct and the builder method `with_arguments()` exists, but `execute_use()` bypasses it — it extracts the argument string separately and injects it directly into the template context. This means:
1. The struct field and builder are currently dead code (no callers of `with_arguments()` in the MCP path).
2. Any code that constructs `UseSkill` programmatically and calls `.execute()` directly will NOT have `arguments` available in the template context, because `execute()` ignores `self.arguments`.

Files:
- `swissarmyhammer-tools/src/mcp/tools/skill/use_op.rs` (lines 30–31: op constructed without arguments)
- `swissarmyhammer-skills/src/operations/use_skill.rs` (execute() does not reference self.arguments)

## Acceptance Criteria
- [ ] Either: `execute_use()` calls `UseSkill::new(name).with_arguments(...)` and `execute()` injects `self.arguments` into the template context, OR the field is removed from `UseSkill` and arguments are passed only through the rendering layer (current approach documented clearly).
- [ ] `with_arguments()` builder either has a caller or is removed.

## Tests
- [ ] Add a test in `swissarmyhammer-tools/src/mcp/tools/skill/mod.rs` invoking `use skill` with `arguments: "fix the login bug"` against the `card` skill and verifying `{{arguments}}` is rendered in the output.
- [ ] `cargo test -p swissarmyhammer-tools` passes. #review-finding #warning