# Implement session management for LlamaAgent

## Location
`swissarmyhammer-workflow/src/agents/llama_agent_executor.rs:480`

## Current State
```rust
/// Clean up abandoned sessions (no-op for now, would be implemented with real session management)
```

## Description
LlamaAgent executor has a placeholder for cleaning up abandoned sessions. Real session management should be implemented to properly track and clean up agent sessions.

## Requirements
- Design session lifecycle management for LlamaAgent
- Track active sessions and their states
- Implement session timeout and cleanup
- Handle graceful session termination
- Add session recovery mechanisms
- Monitor session resource usage
- Add tests for session management scenarios

## Use Cases
- Long-running agent workflows
- Resource cleanup after agent failures
- Session recovery after crashes
- Preventing resource leaks

## Impact
Abandoned sessions may leak resources (memory, connections, etc.).