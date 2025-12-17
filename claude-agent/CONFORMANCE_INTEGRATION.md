# Claude-Agent ACP 0.9.0 Conformance Integration

## Summary

Successfully upgraded claude-agent to agent-client-protocol 0.9.0 and integrated it into the ACP conformance test suite.

## What Was Accomplished

### 1. ACP 0.9.0 Upgrade (214 compilation errors fixed)

**Major API Changes Fixed:**
- **Builder pattern migration**: All non-exhaustive structs now use `.new()` + builder methods
  - `PromptResponse`, `SessionNotification`, `ContentChunk`, `TextContent`
  - `ToolCall`, `ToolCallUpdate`, `PermissionOption`
  - `Error`, `Plan`, `PlanEntry`, `AvailableCommand`
  
- **Tuple variant updates**: Fixed ToolCallContent variants
  - `Content`, `Diff`, `Terminal` now use proper construction
  
- **Field name changes**:
  - `PermissionOption`: `id` → `option_id`
  - `ToolCall`: `id` → `tool_call_id`
  - `BlobResourceContents`: `data` → `blob`
  
- **Type changes**:
  - `meta` fields: `Option<Value>` → `Option<Map<String, Value>>`
  - `Error` construction: struct expression → builder pattern
  - McpServer enum: struct variants → tuple variants

**Files Modified:** agent.rs, tool_types.rs, session.rs, session_loading.rs, session_errors.rs, conversation_manager.rs, protocol_translator.rs, error.rs, plan.rs, tools.rs, capability_validation.rs, content_security_validator.rs, claude.rs, server.rs

### 2. Workspace Integration

**Changes:**
- Removed claude-agent from `exclude` list in root Cargo.toml
- Added to workspace `members`
- Enabled claude-agent dependency in acp-conformance/Cargo.toml
- All 16 conformance tests passing (8 tests × 2 agents)

### 3. Record/Playback Infrastructure

**Design:**
- Added `ClaudeAgentMode` enum to config: Normal, Record, Playback
- Environment variable support: `CLAUDE_AGENT_MODE=record|playback`
- Fixture-based testing infrastructure ready
- Documentation in acp-conformance/RECORD_PLAYBACK.md

**Current Implementation:**
- Infrastructure exists: `RecordedClaudeBackend`, `ClaudeRecorder`
- Fixture directory structure created
- Mode configuration added to AgentConfig
- Test fixture helper in agent_fixtures.rs

**Why Playback Not Fully Wired:**
Current conformance tests (initialization, session management) **don't call the Claude API** - they only test protocol handshakes and session setup. They pass with both agents without needing playback mode.

**When Playback Will Be Needed:**
When conformance tests are added that call `agent.prompt()`, the mode system can be activated by wiring ClaudeClient to use RecordedClaudeBackend instead of ClaudeProcessManager.

## Test Results

```bash
$ cargo nextest run -p acp-conformance
Summary [0.215s] 16 tests run: 16 passed, 0 skipped

Tests passing:
- test_minimal_initialization (llama_agent + claude_agent)
- test_full_capabilities_initialization (llama_agent + claude_agent)
- test_protocol_version_negotiation (llama_agent + claude_agent)
- test_minimal_client_capabilities (llama_agent + claude_agent)
- test_initialize_idempotent (llama_agent + claude_agent)
- test_with_client_info (llama_agent + claude_agent)
- Plus session tests...
```

## Future Work

To fully enable record/playback for API-calling tests:

1. **Modify ClaudeClient** to check `config.mode`:
   - If `Playback`: use `RecordedClaudeBackend::from_file(input_path)`
   - If `Record`: wrap process with `ClaudeRecorder`, save on shutdown
   - If `Normal`: use existing ClaudeProcessManager

2. **Create fixtures** for new tests:
   ```bash
   CLAUDE_AGENT_MODE=record cargo nextest run test_complex_prompt
   ```

3. **Run tests in playback** (default):
   ```bash
   cargo nextest run test_complex_prompt  # Uses fixture, no API calls
   ```

## References

- ACP 0.9.0 spec: https://agentclientprotocol.com/
- Conformance test library: acp-conformance crate
- Example recorded tests: claude-agent/tests/test_*_recorded.rs
