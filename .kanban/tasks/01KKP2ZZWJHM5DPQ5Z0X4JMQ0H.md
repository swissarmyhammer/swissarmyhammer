---
depends_on:
- 01KKP2ZGXYGR9S4KQW7H8BX73W
position_column: done
position_ordinal: ffffffffff8e80
title: Add Claude input structs for new hook events
---
## What
Add input struct definitions for each new hook event type in `avp-common/src/strategy/claude/input.rs`. Update the `HookInput` enum, its `Deserialize` impl, `hook_type()`, and `common()` methods. Update re-exports in `avp-common/src/types/input.rs`.

**New input structs (each with `#[serde(flatten)] pub common: CommonInput` + event-specific fields):**

1. `ElicitationInput` — fields: `mcp_server_name`, `message`, `mode` (string), `requested_schema` (Value)
2. `ElicitationResultInput` — fields: `mcp_server_name`, `action` (string), `content` (Value), `elicitation_id`
3. `InstructionsLoadedInput` — fields: `file_path`, `load_reason` (string), `glob_patterns` (Vec), `memory_type`
4. `ConfigChangeInput` — fields: `source` (string: user_settings/project_settings/local_settings/policy_settings/skills)
5. `WorktreeCreateInput` — fields: `worktree_path`, `branch_name`
6. `WorktreeRemoveInput` — fields: `worktree_path`
7. `PostCompactInput` — (common fields only, mirror PreCompactInput)
8. `TeammateIdleInput` — fields: `teammate_id` (Option)
9. `TaskCompletedInput` — fields: `task_id` (Option), `task_title` (Option)

**Files:**
- `avp-common/src/strategy/claude/input.rs` — structs + HookInput enum + Deserialize + methods
- `avp-common/src/types/input.rs` — add re-exports

## Acceptance Criteria
- [ ] All 9 input structs defined with correct serde attributes
- [ ] `HookInput` enum has all 9 new variants
- [ ] Custom `Deserialize` impl handles all 9 new `hook_event_name` strings
- [ ] `hook_type()` returns correct `HookType` for each
- [ ] `common()` returns `&CommonInput` for each
- [ ] `HookInputType` impl for each new struct
- [ ] Re-exports updated in `types/input.rs`
- [ ] Unknown variant error list in `Deserialize` updated to include all new names

## Tests
Follow the existing patterns in `input.rs` tests (`test_pre_tool_use_input_deserialization`, `test_user_prompt_submit_input`, `test_hook_input_enum`):

- [ ] **Direct struct deserialization for each new type**: JSON blob with all fields → deserialize to specific struct → assert fields. E.g.:
  ```rust
  let json = r#"{"session_id":"abc","transcript_path":"/p","cwd":"/c","permission_mode":"default","hook_event_name":"Elicitation","mcp_server_name":"sah","message":"Pick one","mode":"blocking","requested_schema":{}}"#;
  let input: ElicitationInput = serde_json::from_str(json).unwrap();
  assert_eq!(input.mcp_server_name, "sah");
  ```
- [ ] **HookInput enum dispatch for each new type**: JSON blob → deserialize to `HookInput` → assert `hook_type()` returns correct `HookType` variant. Follows `test_hook_input_enum` pattern.
- [ ] **Optional fields default correctly**: Deserialize with missing optional fields → no error, fields are `None`
- [ ] **AVP schema round-trip**: This is tested in the e2e card, but each struct's `common()` accessor must work
- [ ] Run `cargo test -p avp-common` — all pass