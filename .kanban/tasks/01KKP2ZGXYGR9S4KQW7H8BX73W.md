---
position_column: done
position_ordinal: ffffffffff9a80
title: Add new hook event types to HookType enum
---
## What
Add 9 new variants to the `HookType` enum in `avp-common/src/types/common.rs` and update its `Display` impl. This is the foundation type that all other layers reference.

**New variants:**
- `Elicitation` — MCP server requests user input
- `ElicitationResult` — User responds to MCP elicitation
- `InstructionsLoaded` — CLAUDE.md/rules files loaded
- `ConfigChange` — Config files change
- `WorktreeCreate` — Worktree created
- `WorktreeRemove` — Worktree removed
- `PostCompact` — After context compaction
- `TeammateIdle` — Agent teammate goes idle (already in HookEventKindConfig, missing here)
- `TaskCompleted` — Task marked complete (already in HookEventKindConfig, missing here)

**File:** `avp-common/src/types/common.rs`

## Acceptance Criteria
- [ ] All 9 new variants added to `HookType` enum
- [ ] `Display` impl updated with correct PascalCase strings for each
- [ ] Existing tests pass unchanged

## Tests
Follow the existing pattern in `common.rs` tests (`test_hook_type_serialization`, `test_hook_type_deserialization`):

- [ ] **Serde round-trip for each new variant**: For each of the 9 new variants, test `serde_json::to_string(&HookType::X)` produces `"X"` and `serde_json::from_str::<HookType>("\"X\"")` produces `HookType::X`
- [ ] **CommonInput deserialization with new variants**: Test that a `CommonInput` JSON blob with `hook_event_name: "Elicitation"` (and each other new type) deserializes correctly — follows the pattern of `test_common_input_deserialization`
- [ ] Run `cargo test -p avp-common` — all pass