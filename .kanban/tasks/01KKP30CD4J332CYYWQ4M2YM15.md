---
depends_on:
- 01KKP2ZGXYGR9S4KQW7H8BX73W
position_column: done
position_ordinal: z00
title: Add AVP output types for new hook events
---
## What
Add AVP output types for the 9 new hook events in `avp-common/src/types/avp_output.rs`. Follow existing patterns — some hooks are observe-only (success output), others can block/deny.

**New output types:**

1. `AvpElicitationOutput` — can deny (like PermissionRequest): `allow: bool`, `deny_reason: Option<String>`
2. `AvpElicitationResultOutput` — can block: `allow: bool`, `block_reason: Option<String>`
3. `AvpInstructionsLoadedOutput` — observe-only: base only
4. `AvpConfigChangeOutput` — can block: `allow: bool`, `block_reason: Option<String>`
5. `AvpWorktreeCreateOutput` — can fail creation: `allow: bool`, `deny_reason: Option<String>`
6. `AvpWorktreeRemoveOutput` — observe-only (cannot block): base only
7. `AvpPostCompactOutput` — observe-only (cannot block): base only
8. `AvpTeammateIdleOutput` — can prevent idling: `allow_idle: bool`, `block_reason: Option<String>`
9. `AvpTaskCompletedOutput` — can block completion: `allow: bool`, `block_reason: Option<String>`

Each gets `Default` impl, constructor methods (`allow()`, `deny()`/`block()` where applicable, `*_from_validator()` where applicable).

**File:** `avp-common/src/types/avp_output.rs`

## Acceptance Criteria
- [ ] All 9 output types defined with correct fields
- [ ] Default impls for all (default = allow/success)
- [ ] Constructor methods matching existing patterns
- [ ] Validator-block constructors for types that support blocking

## Tests
Follow the existing patterns in `avp_output.rs` tests (`test_pre_tool_use_allow`, `test_pre_tool_use_deny`, `test_pre_tool_use_deny_from_validator`, `test_stop_allow`, `test_stop_block`):

- [ ] **Allow/success constructor for each type**: Call `allow()` or `success()` → assert `allow == true` (or equivalent), `deny_reason/block_reason == None`, `base.should_continue == true`
- [ ] **Deny/block constructor for blockable types**: Call `deny("reason")` or `block("reason")` → assert `allow == false`, reason field contains expected string. Test for: Elicitation, ElicitationResult, ConfigChange, WorktreeCreate, TeammateIdle, TaskCompleted
- [ ] **Validator-block constructor for blockable types**: Call `deny_from_validator("name", "msg")` or `block_from_validator("name", "msg")` → assert `base.validator_block.is_some()`, validator name and message match. Follows `test_pre_tool_use_deny_from_validator` pattern.
- [ ] **Observe-only types have no deny/block methods**: InstructionsLoaded, WorktreeRemove, PostCompact — only `success()` constructor exists (compile-time guarantee)
- [ ] Run `cargo test -p avp-common` — all pass