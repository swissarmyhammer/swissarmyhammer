---
depends_on:
- 01KKP30V5JHR1JMJT55M1A4K78
position_column: done
position_ordinal: z00
title: Wire HookEvent variants and serialization in HookableAgent
---
## What
Extend the `HookEvent` enum in `hookable_agent.rs` to support the new hook events. Update `kind()`, `matcher_value()`, and `to_base_json()` for each new variant.

**Decision:** All 9 new variants are added as data types for forward compatibility. They can be constructed manually by callers even though the HookableAgent's Agent trait impl doesn't fire them yet (ACP 0.10 has no corresponding lifecycle points).

Add variants:
- `Elicitation { session_id, mcp_server_name, message, mode, requested_schema, cwd }`
- `ElicitationResult { session_id, mcp_server_name, action, content, elicitation_id, cwd }`
- `InstructionsLoaded { file_path, load_reason, cwd }`
- `ConfigChange { session_id, source, cwd }`
- `WorktreeCreate { worktree_path, branch_name, cwd }`
- `WorktreeRemove { worktree_path, cwd }`
- `PostCompact { session_id, cwd }`
- `TeammateIdle { session_id, teammate_id, cwd }`
- `TaskCompleted { session_id, task_id, task_title, cwd }`

Update `kind()`, `matcher_value()`, `to_base_json()` for all 9.

**Matcher semantics:**
- Elicitation/ElicitationResult → `Some(mcp_server_name)`
- InstructionsLoaded → `Some(file_path)` or `Some(load_reason)`
- ConfigChange → `Some(source)`
- WorktreeCreate/WorktreeRemove → `None`
- PostCompact → `None`
- TeammateIdle → `None`
- TaskCompleted → `None`

**File:** `agent-client-protocol-extras/src/hookable_agent.rs`

## Acceptance Criteria
- [ ] All 9 `HookEvent` variants defined with correct fields
- [ ] `kind()` returns correct `HookEventKind` for each
- [ ] `matcher_value()` returns correct values per semantics above
- [ ] `to_base_json()` serializes each variant with correct `hook_event_name` and all fields
- [ ] Existing HookableAgent Agent impl still compiles and works

## Tests
Follow the existing patterns in `avp_schema_tests.rs` (round-trip through `to_command_input_full` → `HookInput` deserialization) and direct unit tests:

- [ ] **AVP schema round-trip for each new variant** (in `avp_schema_tests.rs`): Construct a `HookEvent::Elicitation { ... }` → call `to_command_input_full(&avp_test_context())` → deserialize as `HookInput` → match on `HookInput::Elicitation(inner)` → assert all fields. Repeat for all 9 variants. Follows the exact pattern of `avp_schema_pre_tool_use`, `avp_schema_post_tool_use`, etc.
  ```rust
  #[test]
  fn avp_schema_elicitation() {
      let event = HookEvent::Elicitation {
          session_id: "s1".into(),
          mcp_server_name: "sah".into(),
          message: "Pick option".into(),
          mode: "blocking".into(),
          requested_schema: serde_json::json!({"type":"string"}),
          cwd: PathBuf::from("/tmp"),
      };
      let json = event.to_command_input_full(&avp_test_context());
      let input: HookInput = serde_json::from_value(json).expect("deserialize");
      match input {
          HookInput::Elicitation(inner) => {
              assert_eq!(inner.mcp_server_name, "sah");
              assert_eq!(inner.common.hook_event_name, HookType::Elicitation);
          }
          other => panic!("Wrong variant: {:?}", other.hook_type()),
      }
  }
  ```
- [ ] **kind() unit tests**: For each new variant, construct minimal event → assert `event.kind() == HookEventKind::X`
- [ ] **matcher_value() unit tests**: For matcher-supporting variants (Elicitation, ElicitationResult, InstructionsLoaded, ConfigChange), assert `matcher_value()` returns `Some(expected_string)`. For no-matcher variants (WorktreeCreate, WorktreeRemove, PostCompact, TeammateIdle, TaskCompleted), assert `matcher_value()` returns `None`.
- [ ] **to_base_json() field verification**: For each variant, assert JSON contains `hook_event_name` with correct string value, and all event-specific fields are present
- [ ] Run `cargo test -p agent-client-protocol-extras` — all pass