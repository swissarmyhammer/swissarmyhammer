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

## Proposed Solution

Based on analysis of the existing notify tool implementation and the requirements, I will:

1. **Create separate types module**: Extract `NotifyCreateRequest` from the tool implementation into a dedicated `notify_types.rs` module following the existing pattern used by `memo_types.rs`.

2. **Add proper validation**: Implement validation for:
   - Message not empty (using `McpValidation::validate_not_empty`)
   - Level enum values (with fallback to "info")
   - Context as optional structured JSON

3. **Add comprehensive derives**: Use `Debug`, `Clone`, `Serialize`, `Deserialize`, and `schemars::JsonSchema` for proper serialization and schema generation.

4. **Create enum for notification levels**: Replace the current string-based level with a proper enum to ensure type safety.

5. **Add comprehensive tests**: Include serialization/deserialization tests following the existing test patterns.

## Implementation Steps

1. Create `src/mcp/notify_types.rs` with proper data structures
2. Update the notify tool to use the new types
3. Add validation logic in the tool execution
4. Update imports and module exports
5. Add comprehensive tests for all data types
6. Ensure the tool compiles and tests pass

The new types will follow the exact patterns established in `memo_types.rs` for consistency across the codebase.
## Implementation Details

Successfully implemented all data types and structures for the notify tool according to the requirements.

### Created Files
- **`src/mcp/notify_types.rs`**: New types module containing:
  - `NotifyLevel` enum with Info, Warn, Error variants
  - `NotifyRequest` struct with proper derives and validation
  - Comprehensive test suite (16 tests)

### Key Features Implemented
1. **Type-safe NotifyLevel enum** with:
   - `#[serde(rename_all = "lowercase")]` for proper JSON serialization
   - Case-insensitive string conversion (`From<&str>` and `From<Option<String>>`)
   - Default to "info" for invalid values
   - Conversion to `&'static str`

2. **Comprehensive NotifyRequest struct** with:
   - Required `message` field (validated as non-empty)
   - Optional `level` field (defaults to `None`/info)
   - Optional `context` field for structured JSON data
   - Builder pattern methods (`new()`, `with_level()`, `with_context()`)
   - Built-in `validate()` method
   - Typed `get_level()` method that returns NotifyLevel enum

3. **Enhanced validation**:
   - Built-in validation in `NotifyRequest::validate()`
   - Integration with existing `McpValidation::validate_not_empty()`
   - Clear error messages for validation failures

4. **Proper serialization support**:
   - All required derives: `Debug`, `Clone`, `Serialize`, `Deserialize`, `schemars::JsonSchema`
   - JSON Schema support for MCP protocol
   - Backward-compatible serialization format

### Updated Files
- **`notify/create/mod.rs`**: Updated to use new types and enhanced validation
- **`src/mcp/mod.rs`**: Added notify_types module export and register_notify_tools re-export
- **`src/lib.rs`**: Added register_notify_tools to public exports

### Test Coverage
- 16 comprehensive tests for notify_types module
- 14 existing tests for notify/create module still passing
- **Total: 30 tests passing** for notify functionality
- Tests cover serialization/deserialization, validation, enum conversions, edge cases, and unicode support

### Code Quality
- All code formatted with `cargo fmt`
- Passes `cargo clippy` checks (no warnings in notify-related code)
- Follows existing codebase patterns and conventions
- Full compilation successful with no errors

## Success Criteria Met
✅ Data structures compile without errors  
✅ Proper JSON serialization/deserialization works  
✅ Basic validation logic is in place  
✅ Structures are ready for MCP trait implementation  
✅ Follows existing validation patterns from other MCP tools  
✅ Use serde for JSON serialization/deserialization  
✅ Ensure proper error handling for invalid inputs  
✅ Use Option types for optional parameters with sensible defaults  

The notify tool now has a robust, type-safe foundation for handling notification requests with comprehensive validation and testing.