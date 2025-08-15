# Implement Logging Integration for Notify Tool

Refer to /Users/wballard/github/swissarmyhammer/ideas/notify_tool.md

## Objective
Implement the core logging functionality using the tracing system with the "llm_notify" target as specified in the requirements.

## Tasks
1. Implement the logging logic in the execute method
2. Use appropriate tracing macros (info!, warn!, error!) based on level
3. Include context data in structured logging
4. Handle level validation with fallback to "info"
5. Ensure thread-safe logging implementation

## Logging Implementation Requirements

### Core Logic
```rust
match request.level.as_deref().unwrap_or("info") {
    "info" => tracing::info!(target: "llm_notify", context = request.context, "{}", request.message),
    "warn" => tracing::warn!(target: "llm_notify", context = request.context, "{}", request.message),
    "error" => tracing::error!(target: "llm_notify", context = request.context, "{}", request.message),
    _ => tracing::info!(target: "llm_notify", context = request.context, "{}", request.message),
}
```

### Implementation Details
- Use "llm_notify" as the logging target for filtering
- Include structured context data in log entries
- Handle None/invalid levels by defaulting to "info"
- Ensure logging failures don't cause tool execution to fail
- Use appropriate tracing structured fields

## Error Handling
- Logging failures should be handled gracefully
- Tool should not fail if logging encounters issues
- Maintain proper error response format
- Log any internal logging errors at debug level

## Success Criteria
- Logging works correctly for all three levels (info, warn, error)
- Context data is properly included in log entries
- "llm_notify" target is used correctly for filtering
- Tool execution succeeds even if logging fails
- Invalid levels default to "info" correctly

## Dependencies
- Build on McpTool implementation from step 000003