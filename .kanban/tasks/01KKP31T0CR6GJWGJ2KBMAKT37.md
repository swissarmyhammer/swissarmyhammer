---
depends_on:
- 01KKP31C4G0AK64C7186MAGXWB
position_column: done
position_ordinal: ffffffffda80
title: Update e2e hook tests for new event types
---
## What
Update the e2e hook tests in `agent-client-protocol-extras/tests/e2e_hooks/` to cover the new hook event types. These tests use the full hook infrastructure — shell scripts, PlaybackAgent, broadcast channels, stdin capture — to verify end-to-end behavior.

**Note:** Since the new hook types are forward-compatible only (ACP 0.10 can't fire them from the Agent trait), e2e tests focus on:
1. Config parsing and silent skip behavior
2. Manual event construction and JSON serialization
3. AVP schema round-trip (already covered in Card 5, but verify no regressions)

**Files:** `agent-client-protocol-extras/tests/e2e_hooks/*.rs`

## Acceptance Criteria
- [ ] Config deserialization tests for all new `HookEventKindConfig` variants
- [ ] Forward-compatible variants tested (config accepted, silently skipped at runtime)
- [ ] Exhaustive variant tracking updated
- [ ] Full test suite passes with no regressions

## Tests
Follow the existing patterns in the e2e test files:

- [ ] **Forward-compat config acceptance** (in `hook_edge_case_tests.rs`): Create a HookConfig with `Elicitation` event and a command hook → build_hookable_agent() → no panic, no error. Run a prompt → hook does NOT fire (it's forward-compat). Follows the pattern where unsupported events are silently skipped.
  ```rust
  #[tokio::test]
  async fn forward_compat_elicitation_config_accepted() {
      let tmp = tempfile::TempDir::new().unwrap();
      let script = write_exit_script(tmp.path(), "hook.sh", 0, "");
      let config = hook_config_json("Elicitation", script.to_str().unwrap(), None);
      let playback = load_playback_agent("tool_call_session.json");
      let agent = build_hookable_agent(Arc::new(playback), &config);
      let session_id = init_session(&agent).await;
      // Prompt should succeed — Elicitation hook is silently skipped
      let result = run_prompt(&agent, session_id, "test").await;
      assert!(result.is_ok());
      // Hook should NOT have fired
      let captured = read_stdin_capture(tmp.path(), "hook.sh");
      assert!(captured.is_none());
  }
  ```
  Repeat for: ElicitationResult, InstructionsLoaded, ConfigChange, WorktreeCreate, WorktreeRemove.

- [ ] **Update `all_event_kinds()` exhaustive match** (in `cross_cutting_tests.rs`): Add `HookEventKind::PostCompact`, `HookEventKind::TeammateIdle`, `HookEventKind::TaskCompleted` to the exhaustive match. Update length assertion from 7 to 10.

- [ ] **Mixed config with supported and unsupported events**: Create a HookConfig with both `PreToolUse` (supported) and `Elicitation` (forward-compat) → build agent → PreToolUse hook fires normally, Elicitation is silently ignored.

- [ ] Run `cargo test -p agent-client-protocol-extras` — all pass
- [ ] Run `cargo test --workspace` — no regressions across all crates