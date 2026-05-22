# ACP Conformance Test Suite

Official conformance testing for Agent Client Protocol (ACP) implementations.

## Overview

This crate provides comprehensive conformance testing based on the official ACP specification at https://agentclientprotocol.com. Tests verify that agent implementations correctly follow the protocol requirements.

## Test Coverage

### ✅ Initialization Protocol (Complete)

Based on https://agentclientprotocol.com/protocol/initialization

### ✅ Session Setup Protocol (Complete)

Based on https://agentclientprotocol.com/protocol/session-setup

**Requirements Tested:**
1. ✅ Protocol version negotiation (V1)
2. ✅ Client capabilities handling (fs, terminal)
3. ✅ Agent capabilities advertisement (loadSession, prompt types, MCP transports)
4. ✅ Authentication methods declaration
5. ✅ Implementation info (agent name, version)
6. ✅ Initialize idempotency
7. ✅ Client info handling

**Test Functions:**
- `test_minimal_initialization` - Basic protocol negotiation with no capabilities
- `test_full_capabilities_initialization` - Full client capability advertisement
- `test_minimal_client_capabilities` - Missing capability handling
- `test_protocol_version_negotiation` - Version handling
- `test_initialize_idempotent` - Multiple initialize calls
- `test_with_client_info` - Client implementation info

**Requirements Tested:**
1. ✅ Protocol version negotiation (V1)
2. ✅ Client capabilities handling (fs, terminal)
3. ✅ Agent capabilities advertisement (loadSession, prompt types, MCP transports)
4. ✅ Authentication methods declaration
5. ✅ Implementation info (agent name, version)
6. ✅ Initialize idempotency
7. ✅ Client info handling

**Test Functions:**
- `test_new_session_minimal` - Create session with minimal params (cwd only)
- `test_new_session_with_mcp` - Create session with MCP server config
- `test_session_ids_unique` - Verify unique session IDs
- `test_load_nonexistent_session` - Error handling for invalid session
- `test_set_session_mode` - Mode switching

**Requirements Tested:**
1. ✅ session/new creates sessions with unique IDs
2. ✅ cwd parameter accepted
3. ✅ mcpServers parameter accepted
4. ✅ session/load rejects nonexistent sessions
5. ✅ session/set-mode accepts mode changes

### 🚧 Other Protocol Sections (TODO)

- Prompting
- Tool calls
- Notifications
- Cancellation

## Running Tests

```bash
# Run all conformance tests
cargo test -p acp-conformance

# Run initialization tests only
cargo test -p acp-conformance initialization

# Run with detailed output
cargo test -p acp-conformance -- --nocapture
```

## Test Results Summary

**Total Tests:** 11 (1 lib + 4 mock + 6 llama-agent)
**Passing:** 11/11 ✅
**Failing:** 0
**Ignored:** 2 (old process-spawn stubs)

### llama-agent

✅ **6/6 initialization tests passing**

- ✅ test_llama_minimal_initialization
- ✅ test_llama_full_capabilities
- ✅ test_llama_protocol_version
- ✅ test_llama_minimal_client_caps
- ✅ test_llama_initialize_idempotent
- ✅ test_llama_with_client_info

**Capabilities Detected:**
- ✓ Session loading supported
- ✓ Image content
- ✓ Audio content
- ✓ Embedded context
- ✓ HTTP transport (MCP)
- ✗ SSE transport (not supported)

### claude-agent

⚠️ **Cannot test - breaking API changes**

Workspace has been upgraded to agent-client-protocol 0.9.0, but claude-agent has 229 compilation errors due to breaking changes in the protocol:
- Non-exhaustive struct construction changes (E0639)
- Field name changes (E0560)
- Type signature changes (E0308)
- Pattern matching changes (E0004)

**Action Required:** Fix claude-agent to work with agent-client-protocol 0.9.0 API changes

**Note:** claude-agent is in `../claude-agent` (sibling directory), excluded from workspace build

### mock-agent

✅ **4/4 initialization tests passing**

Simple mock implementation used for testing the test framework itself.

## Architecture

```
┌─────────────────────────────────────┐
│  Conformance Test Suite             │
│  (acp-conformance crate)             │
└──────────────┬──────────────────────┘
               │ Uses Agent trait
               │
┌──────────────┴──────────────────────┐
│  ACP Client                          │
│  (agent-client-protocol crate)       │
│  - ClientSideConnection              │
│  - AgentSideConnection               │
└──────────────┬──────────────────────┘
               │ JSON-RPC over streams
               │
┌──────────────┴──────────────────────┐
│  Agent Implementations               │
│  - llama-agent::acp::AcpServer       │
│  - claude-agent::ClaudeAgent         │
│    (served via swissarmyhammer-agent)│
└─────────────────────────────────────┘
```

## Test Methodology

1. **Stream-Based Testing**: Tests create bidirectional streams (using `piper::pipe`) to connect the client and agent without requiring process spawning
2. **In-Process Testing**: Agents run in the same process for faster, more reliable testing
3. **LocalSet Execution**: Uses tokio LocalSet for non-Send futures required by the ACP protocol
4. **Serial Execution**: Tests run serially to avoid llama.cpp backend conflicts

## Protocol Compliance Validation

Each test validates:
- ✅ JSON-RPC 2.0 format (via agent-client-protocol crate)
- ✅ camelCase field names (enforced by protocol crate)
- ✅ Required fields present in responses
- ✅ Optional fields handled gracefully
- ✅ Error handling for invalid inputs
- ✅ Consistent behavior across multiple calls

## Next Steps

1. Add session management tests (new, load, set_mode)
2. Add prompt handling tests
3. Add tool call tests
4. Add notification tests
5. Add cancellation tests
6. Update claude-agent to compatible ACP version and enable testing

## References

- [ACP Specification](https://agentclientprotocol.com)
- [ACP GitHub](https://github.com/zed-industries/agent-client-protocol)
- [Initialization Protocol](https://agentclientprotocol.com/protocol/initialization)
