AgentExecutor.execute_prompt returning a json Value is really shitty compared to returning an actual type.

## Proposed Solution

After analyzing the codebase, the issue is that `AgentExecutor.execute_prompt` currently returns `ActionResult<Value>` where `Value` is `serde_json::Value`. This is problematic because:

1. **Type Safety**: `serde_json::Value` is a generic JSON type that could be anything (String, Number, Object, Array, Boolean, Null)
2. **Poor API Design**: Callers have to pattern match and handle all possible JSON types when they really expect structured agent responses
3. **Runtime Errors**: Callers currently do unsafe extraction like `Value::String(s) => s.clone()` with fallbacks to JSON serialization

### Current Usage Pattern
Callers currently do:
```rust
let response_text = match &response {
    Value::String(s) => s.clone(),
    _ => serde_json::to_string(&response).unwrap_or_default(),
};
```

### Proposed New Type
Replace `serde_json::Value` with a proper `AgentResponse` struct:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    /// The primary response content from the agent
    pub content: String,
    /// Optional metadata about the response
    pub metadata: Option<serde_json::Value>,
    /// Response status/type for different kinds of responses
    pub response_type: AgentResponseType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentResponseType {
    /// Standard text response
    Success,
    /// Partial response (streaming, timeout, etc.)
    Partial,
    /// Error response with error details
    Error,
}
```

### Implementation Plan
1. Define the new `AgentResponse` type in the actions module
2. Update the `AgentExecutor` trait to return `ActionResult<AgentResponse>`
3. Update `ClaudeCodeExecutor` to return the new type
4. Update `LlamaAgentExecutor` to return the new type  
5. Update all callers to use the new structured type
6. Remove the unsafe pattern matching on generic JSON values

This provides:
- **Type Safety**: Callers know exactly what structure they're getting
- **Extensibility**: Can add metadata and different response types
- **Better Error Handling**: Clear distinction between success and error responses
- **API Clarity**: Much clearer what the method returns and how to use it
## Code Review Completion

Completed comprehensive code review and addressed all identified issues:

### ✅ Issues Fixed

1. **Lint Errors**: Fixed unused `AgentResponseType` import by moving it to the test module where it's actually used
2. **Code Formatting**: Ran `cargo fmt --all` to fix all formatting violations  
3. **Test Coverage**: Added 5 comprehensive unit tests for `AgentResponse`:
   - `test_agent_response_success()` - Tests success constructor and methods
   - `test_agent_response_success_with_metadata()` - Tests success with metadata
   - `test_agent_response_error()` - Tests error constructor and methods  
   - `test_agent_response_partial()` - Tests partial constructor and methods
   - `test_agent_response_serialization()` - Tests JSON serialization/deserialization

### ✅ Issues Assessed

4. **Backward Compatibility**: Reviewed the JSON conversion pattern - this is working as designed. The `AgentExecutor.execute_prompt()` method now returns structured `AgentResponse`, but the `Action.execute()` trait still uses `Value` for compatibility. The conversion is necessary and correct.
5. **Documentation**: Verified all `AgentResponse` methods already have comprehensive doc comments

### ✅ Verification

- All tests pass (`cargo test test_agent_response` - 5/5 passing)
- No lint warnings (`cargo clippy` clean) 
- Code formatting correct (`cargo fmt --check` clean)
- All identified code review tasks completed

The AgentResponse refactoring successfully improves type safety and API design as intended by the original issue while maintaining backward compatibility with the existing Action trait system.