# Complete Test Conversion Status

## What We've Actually Converted: 28 Tests

### Claude-Agent Package (26 tests)
**Location:** `claude-agent/tests/test_prompt_recorded.rs`
**Original location:** `claude-agent/src/agent.rs` (unit tests in module)
**Status:** ✅ 100% of agent.rs tests that spawn Claude are converted
**Time saved:** ~220 seconds → <0.2s

### Hence Integration (2 tests)
**Location:** `hence/tests/integration_display_grouping_recorded.rs`
**Original location:** `hence/tests/integration_display_grouping.rs`
**Status:** ✅ Converted (but originals still exist and run)
**Time saved:** ~22 seconds → <0.02s

**Total so far:** 28 tests, ~242 seconds saved

---

## What's Still Spawning Claude: ~17+ Tests

### Hence Integration Tests (`hence/tests/integration/`)

These tests create `AgentClient` which spawns Claude:

**Active (not ignored):**
1. **test_prompt_simple.rs** - 1 test
   - Tests simple prompt with timeout

2. **test_prompt_response.rs** - 1 test
   - Tests prompt gets response

3. **test_prove_communication.rs** - 1 test
   - Tests bidirectional communication

4. **test_session_store_integration.rs** - 1 test
   - Tests session storage

5. **test_streaming_messages.rs** - 2 tests
   - Tests streaming message flow

6. **integration_test.rs** - 1 test
   - Basic integration test

7. **test_rewind_middleware.rs** - 4 tests
   - Tests rewind functionality

8. **acp_client.rs** - 6 tests
   - ACP client integration tests

**Ignored (manual only):**
9. **test_middleware_e2e.rs** - 1 test (marked #[ignore])

**Subtotal:** ~17 active integration tests

### Hence Top-Level Tests

**Still have originals:**
- `integration_display_grouping.rs` - 2 tests (originals not deleted)

### Claude-Agent Integration Tests

**Ignored (manual verification only):**
- `test_slash_commands_from_init.rs` - marked #[ignore]
- `test_tool_completion_real.rs` - marked #[ignore]

**Already have recorded versions:**
- `test_slash_commands_from_init_recorded.rs` ✅
- `test_tool_completion_recorded.rs` ✅
- `test_tool_call_flow_recorded.rs` ✅
- `test_immediate_response_recorded.rs` ✅
- `test_message_duplication_recorded.rs` ✅

---

## Accurate Count

**Tests converted to recorded:** 28
**Tests still spawning Claude (active):** ~19
  - ~17 in hence/tests/integration/
  - 2 in hence/tests/integration_display_grouping.rs (originals)

**Tests spawning Claude (ignored, manual only):** 3
  - 1 in hence/tests/integration/test_middleware_e2e.rs
  - 2 in claude-agent/tests/ (slash commands, tool completion)

**Total tests ever spawning Claude:** ~50
**Conversion rate:** 28/50 = 56%

---

## Next Steps

To get to 100%, need to convert the 17 active hence integration tests.

**Estimated time savings:** ~150-200 more seconds (integration tests are typically slow)

