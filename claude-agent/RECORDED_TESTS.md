# Recorded Tests

Tests using `RecordedClaudeBackend` to avoid spawning real Claude binaries.

## Converted Tests (56 total!)

**Claude-Agent (26):**
1. test_recorded_backend_basic_flow - Basic prompt (8s → 0.01s)
2. test_conversation_context_maintained_recorded - Multi-turn (15s → 0.01s)
3. test_full_prompt_flow_recorded - Complete flow (10s → 0.01s)
4. test_streaming_prompt_recorded - Streaming chunks (12s → 0.01s)
5. test_non_streaming_fallback_recorded - Non-streaming (10s → 0.01s)
6. test_streaming_with_resource_link_recorded - Resource links (13s → 0.01s)
7. test_new_session_recorded - Session creation (5s → 0.01s)
8. test_load_session_with_history_recorded - History replay (12s → 0.01s)
9. test_streaming_session_context_maintained_recorded - Context retention (56s → 0.01s) ⭐
10. test_prompt_validation_empty_prompt_recorded - Empty prompt validation (8s → 0.01s)
11. test_prompt_validation_non_text_content_recorded - Content validation (8s → 0.01s)
12. test_load_session_recorded - Load session (8s → 0.01s)
13. test_set_session_mode_recorded - Set mode (8s → 0.01s)
14. test_full_protocol_flow_recorded - Full protocol (12s → 0.01s)
15. test_request_permission_basic_recorded - Permission flow (8s → 0.01s)
16. test_load_session_capability_validation_recorded - Capability check (10s → 0.01s)
17. test_user_message_chunks_sent_on_prompt_recorded - Message chunking (10s → 0.01s)
18. test_prompt_validation_invalid_session_id_recorded - Invalid ID (5s → 0.01s)
19. test_prompt_nonexistent_session_recorded - Nonexistent session (5s → 0.01s)
20. test_request_permission_generates_default_options_recorded - Default options (8s → 0.01s)
21. test_load_nonexistent_session_recorded - Nonexistent load (5s → 0.01s)
22. test_streaming_capability_detection_recorded - Capability detection (8s → 0.01s)
23. test_streaming_prompt_enforces_turn_request_limit_recorded - Turn limit (8s → 0.01s)
24. test_new_session_validates_mcp_transport_recorded - MCP new session (7s → 0.01s)
25. test_load_session_validates_mcp_transport_recorded - MCP load session (7s → 0.01s)
26. test_recorded_backend_exhaustion - Backend test (n/a)

**Hence Integration - Display Grouping (2):**
27. test_display_grouping_with_recorded_claude_messages - Display grouping (11s → 0.01s) ⭐
28. test_grouping_preserves_all_messages_recorded - Message preservation (11s → 0.01s) ⭐

**Hence Integration - Prompts/Sessions (13):**
29-41. All prompt, session, and ACP client tests (integration_prompt_recorded.rs)

**Hence Integration - Rewind Middleware (4):**
42-45. All rewind middleware tests (integration_rewind_recorded.rs)

**Hence Integration - Postprompt Chain (11):**
46-56. All postprompt chain tests (integration_postprompt_recorded.rs)

**Total: 56 tests, ~500s → <0.5s (~1,000x faster!)**

## ✅ All Active Tests That Spawn Claude Are Now Converted!

🎯 **100% of active tests spawning Claude are recorded!**

Original tests remain for manual verification but normal CI runs use recorded versions.

## Usage

**Run all recorded tests:**
```bash
# Claude-agent tests (26 tests)
cargo test --package claude-agent --test test_prompt_recorded

# Hence integration tests (30 tests total)
cargo test --package hence --test integration_display_grouping_recorded
cargo test --package hence --test integration_prompt_recorded
cargo test --package hence --test integration_rewind_recorded
cargo test --package hence --test integration_postprompt_recorded
```

**Find tests that still spawn Claude:**
```bash
RUST_LOG=claude_agent=warn cargo test 2>&1 | grep "🚨 SPAWNING"
```

**Check for process leaks:**
```bash
ps aux | grep -E "claude.*--print.*stream-json" | wc -l
# Should be: 0
```

## How It Works

See comments in:
- `src/claude_backend.rs` - RecordedClaudeBackend implementation
- `tests/common/recording.rs` - ClaudeRecorder helper
- `tests/test_prompt_recorded.rs` - Example tests
