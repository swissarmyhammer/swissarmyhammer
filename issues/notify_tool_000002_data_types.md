# Define Notify Tool Data Types and Structures

Refer to /Users/wballard/github/swissarmyhammer/ideas/notify_tool.md

## Objective
Define the core data structures for the notify tool including request parameters, validation, and response types.

## Tasks
1. Create `NotifyRequest` struct with proper field definitions
   - `message: String` (required)
   - `level: Option<String>` (enum: "info", "warn", "error", default: "info")
   - `context: Option<Value>` (structured JSON data)
2. Create `NotifyTool` struct implementing the tool
3. Add proper serialization/deserialization derives
4. Add validation for message (non-empty) and level (valid enum values)
5. Define error types for validation failures

## Data Structure Requirements

### NotifyRequest
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotifyRequest {
    pub message: String,
    #[serde(default)]
    pub level: Option<String>,
    #[serde(default)]
    pub context: Option<serde_json::Value>,
}
```

### Validation Rules
- Message must not be empty (minLength: 1)
- Level must be one of: "info", "warn", "error" (default to "info" if invalid)
- Context can be any valid JSON structure

## Implementation Notes
- Use serde for JSON serialization/deserialization
- Follow existing validation patterns from other MCP tools
- Ensure proper error handling for invalid inputs
- Use Option types for optional parameters with sensible defaults

## Success Criteria
- Data structures compile without errors
- Proper JSON serialization/deserialization works
- Basic validation logic is in place
- Structures are ready for MCP trait implementation

## Dependencies
- Build on the basic structure from step 000001