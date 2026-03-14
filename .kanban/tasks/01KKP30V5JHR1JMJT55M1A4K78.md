---
depends_on:
- 01KKP2ZGXYGR9S4KQW7H8BX73W
position_column: done
position_ordinal: z00
title: Expand HookEventKindConfig and HookEventKind enums
---
## What
Update both hook event enums in `agent-client-protocol-extras` to include all new hook types.

**`HookEventKindConfig`** in `hook_config.rs` — add missing variants:
- `Elicitation`
- `ElicitationResult`
- `InstructionsLoaded`
- `ConfigChange`
- `WorktreeCreate`
- `WorktreeRemove`
- `PostCompact`

(TeammateIdle and TaskCompleted already exist here)

**`HookEventKind`** in `hookable_agent.rs` — currently only has 7 variants (SessionStart through Notification). Add all new variants that can be fired from ACP lifecycle:
- `PostCompact` (fires after compaction, no ACP equivalent but useful)
- `TeammateIdle`
- `TaskCompleted`

The remaining new types (Elicitation, ElicitationResult, InstructionsLoaded, ConfigChange, WorktreeCreate, WorktreeRemove) stay as forward-compatible in `HookEventKindConfig` only — they map to `Err(UnsupportedEventKind)` in the `TryFrom` impl because ACP 0.10 has no lifecycle point to fire them from.

**Update `TryFrom<HookEventKindConfig> for HookEventKind`** to route the new config variants correctly.

**Files:**
- `agent-client-protocol-extras/src/hook_config.rs`
- `agent-client-protocol-extras/src/hookable_agent.rs`

## Acceptance Criteria
- [ ] `HookEventKindConfig` has all 16 variants (7 existing active + 2 existing forward-compat + 7 new)
- [ ] `HookEventKind` gains PostCompact, TeammateIdle, TaskCompleted (10 total)
- [ ] `TryFrom` correctly maps the 3 new active variants, returns `Err` for the 6 forward-compat ones
- [ ] Serde round-trip works for all `HookEventKindConfig` variants

## Tests
Follow the existing patterns in `hook_config.rs` tests (`test_json_command_hook`) and the exhaustive variant pattern in `cross_cutting_tests.rs`:

- [ ] **Serde round-trip for each new `HookEventKindConfig` variant**: Serialize to JSON string, deserialize back, assert equality. Test all 7 new config variants.
- [ ] **Config deserialization with new event names**: Build a `HookConfig` JSON with each new event name as key → deserialize → assert the event key is present in the `hooks` map. Follows `test_json_command_hook` pattern:
  ```rust
  let json = r#"{"hooks":{"Elicitation":[{"hooks":[{"type":"command","command":"./check.sh"}]}]}}"#;
  let config: HookConfig = serde_json::from_str(json).unwrap();
  assert!(config.hooks.contains_key(&HookEventKindConfig::Elicitation));
  ```
- [ ] **TryFrom for new active variants**: `HookEventKindConfig::PostCompact.try_into()` → `Ok(HookEventKind::PostCompact)`, same for TeammateIdle, TaskCompleted
- [ ] **TryFrom for new forward-compat variants**: `HookEventKindConfig::Elicitation.try_into()` → `Err(UnsupportedEventKind)`, same for ElicitationResult, InstructionsLoaded, ConfigChange, WorktreeCreate, WorktreeRemove
- [ ] **Forward-compat variants silently skipped in build_registrations()**: Build a HookConfig with an `Elicitation` event → call `build_registrations()` → no error, no registration created (silently skipped). This follows the existing behavior for PermissionRequest/SubagentStart etc.
- [ ] **Update `all_event_kinds()` in cross_cutting_tests.rs**: Add new HookEventKind variants to the exhaustive match and update the length assertion (7 → 10)
- [ ] Run `cargo test -p agent-client-protocol-extras` — all pass