# Add create_todo Parameter to rules_check Tool

## Problem

Currently, the `are_rules_passing` prompt manually creates todos for each rule violation by calling `todo_create` after running `rules_check`. This adds complexity to the prompt and requires parsing the rules check output to extract violation details.

## Proposed Solution

Add a `create_todo: bool` parameter to the `rules_check` MCP tool that, when enabled, automatically creates a todo item for each violation found.

## Requirements

### Parameter Addition
- Add `create_todo` optional parameter to `RuleCheckRequest` struct
- Default to `false` to maintain backward compatibility
- Document the parameter in the tool's description.md

### Todo Creation Logic
When `create_todo` is `true`:
- For each rule violation found, call `todo_create` inline
- Create one todo per violation (not one todo for all violations)

### Todo Content Requirements

Each todo should include rich context:

**Task field** should contain:
```
Fix [rule_name] violation in [file_path]:[line_number]
```

**Context field** should contain:
```markdown
## Rule Violation

**Rule**: [rule_name]
**File**: [file_path]
**Line**: [line_number]
**Severity**: [severity]

## Violation Details

[violation message/output from rule check]

## How to Fix

[suggestion from rule if available]
```

### Error Handling
- If todo creation fails, log a warning but don't fail the rules check
- Continue checking other rules even if todo creation fails
- Return both violation information AND todo creation results

### Response Format

When `create_todo: true`, the response should include:
```json
{
  "violations": [...existing violation data...],
  "todos_created": [
    {
      "todo_id": "01XXXXX",
      "rule": "rule-name",
      "file": "path/to/file.rs",
      "line": 42
    }
  ]
}
```

## Benefits

1. **Simpler Prompts**: `are_rules_passing` prompt becomes much simpler - just pass `create_todo: true`
2. **Better Context**: Tool has direct access to full violation details for richer todos
3. **Atomic Operation**: Todo creation happens as part of the check, not as a separate step
4. **Progress Notifications**: Can send notifications as todos are created
5. **Consistency**: All violations get todos with consistent format

## Implementation Notes

- Access `ToolContext.progress_sender` to send notifications when todos are created
- Use the existing `todo_create` functionality (may need to refactor into a reusable function)
- Consider adding a `max_todos` parameter to limit todo creation (aligned with `max_errors`)

## Testing

- Test with `create_todo: false` (default) - should work as before
- Test with `create_todo: true` - should create todos for violations
- Test that todo context includes all required information
- Test error handling when todo creation fails
- Verify progress notifications are sent

## Related

This supports the new review workflow where `are_rules_passing` creates todos for violations that are then processed by the `do_todos` workflow.



## Proposed Solution

I'll implement the `create_todo` parameter by following these steps:

### 1. Add Parameter to RuleCheckRequest (mod.rs:52-68)
- Add `create_todo: Option<bool>` field to the MCP `RuleCheckRequest` struct
- Mark it as `#[serde(skip_serializing_if = "Option::is_none")]` for optional handling
- Default behavior: `None` → `false` (backward compatible)

### 2. Update JSON Schema (mod.rs:339-374)
- Add `create_todo` property to the schema with:
  - Type: `boolean`
  - Description: "Automatically create a todo item for each rule violation found"
  - Not required (optional parameter)

### 3. Create Todo Helper Function
- Create a new private async function `create_todo_for_violation()` that:
  - Takes a `RuleViolation` and creates a `TodoItem`
  - Formats the task field: `"Fix [rule_name] violation in [file_path]:[line_number]"`
  - Formats the context field with markdown containing:
    - Rule name, file, line, severity
    - Violation message
    - Suggestion (if available)
  - Uses `TodoStorage::new_default()` to access todo functionality
  - Calls `storage.create_todo_item(task, Some(context))`
  - Returns the created `TodoId` on success
  - Logs warnings but doesn't fail if todo creation fails

### 4. Update Execute Method (mod.rs:376-570)
- After collecting violations from the stream (mod.rs:483-503)
- If `request.create_todo` is `Some(true)`:
  - Iterate through violations
  - Call `create_todo_for_violation()` for each one
  - Track created todo IDs in a `Vec<(TodoId, String, PathBuf, usize)>` for the response
  - Send progress notifications as todos are created
  - Continue on error (log warning, don't fail the check)

### 5. Update Response Format (mod.rs:505-570)
- Add new `todos_created` array to response metadata when todos were created
- Include todo ID, rule name, file path, and line number for each created todo
- Update the result text to mention todos were created if `create_todo` was true

### 6. Update Tool Description (description.md)
- Add documentation for the new `create_todo` parameter
- Include examples showing usage with `create_todo: true`
- Explain the todo format and content structure

### 7. Add Tests
- Test with `create_todo: false` (default) - should work as before
- Test with `create_todo: true` - should create todos for violations
- Test that todo context includes all required information
- Test error handling when todo creation fails (graceful degradation)
- Verify todos are created with proper ULIDs and sequencing

### Implementation Notes

- **No Dependency Changes**: `swissarmyhammer-todo` is already a dependency via `swissarmyhammer-tools/Cargo.toml`
- **Atomic Operations**: Todo creation happens after violation collection, so rule checking isn't slowed down
- **Error Isolation**: Todo creation failures won't fail the rule check - we log and continue
- **Progress Notifications**: Use existing `context.progress_sender` to notify about todo creation progress
- **Backward Compatibility**: Default behavior unchanged when parameter is omitted




## Implementation Complete

Successfully implemented the `create_todo` parameter for the `rules_check` MCP tool.

### Changes Made

1. **Parameter Addition** (mod.rs:69-71)
   - Added `create_todo: Option<bool>` field to `RuleCheckRequest` struct
   - Marked as `#[serde(skip_serializing_if = "Option::is_none")]`
   - Defaults to `None` which is treated as `false` (backward compatible)

2. **JSON Schema Update** (mod.rs:367-370)
   - Added `create_todo` property to the tool schema
   - Type: boolean
   - Description: "Automatically create a todo item for each rule violation found (default: false)"

3. **Helper Function** (mod.rs:158-210)
   - Implemented `create_todo_for_violation()` async function
   - Takes a `RuleViolation` and creates a formatted todo
   - Task format: `"Fix [rule_name] violation in [file_path]"`
   - Context includes: Rule name, file path, severity, violation message, and guidance
   - Uses `TodoStorage::new_default()` to create todos
   - Gracefully handles errors without failing the rule check

4. **Execute Method Updates** (mod.rs:581-631)
   - After collecting violations, checks if `create_todo` is enabled
   - Iterates through violations and creates todos
   - Tracks created todo IDs in `todos_created` vector
   - Sends progress notifications for each todo created
   - Logs warnings for todo creation failures (doesn't fail the check)
   - Updates completion notification to include todo count

5. **Tool Description** (description.md)
   - Added comprehensive parameter documentation
   - Included examples showing usage with `create_todo: true`
   - Documented the todo format and content structure

6. **Tests Added** (mod.rs:819-920)
   - `test_rule_check_request_with_create_todo`: Tests parameter parsing
   - `test_rule_check_request_create_todo_default`: Tests default None behavior
   - `test_rule_check_with_create_todo`: Integration test for todo creation
   - `test_rule_check_create_todo_error_handling`: Tests graceful error handling

### Key Design Decisions

1. **No Line Numbers**: The `RuleViolation` struct doesn't have a `line_number` field, only `file_path`, `rule_name`, `severity`, and `message`. The message field contains the full LLM response with violation details.

2. **Error Isolation**: Todo creation failures are logged as warnings but don't fail the rule check. This ensures rule checking completes successfully even if todo storage has issues.

3. **Progress Notifications**: Each todo creation sends a progress notification, allowing clients to track the todo creation process.

4. **Backward Compatibility**: The parameter defaults to `None`/`false`, so existing code continues to work unchanged.

5. **Atomic Operation**: Todos are created after all violations are collected, so rule checking isn't slowed down by todo creation.

### Test Results

All 621 tests pass, including the new tests for the `create_todo` functionality:
- ✅ Parameter parsing tests pass
- ✅ Integration test for todo creation passes  
- ✅ Error handling test passes (completes successfully even with storage errors)
- ✅ All existing tests continue to pass (backward compatibility maintained)

### Usage Example

```json
{
  "rule_names": ["code-quality/cognitive-complexity"],
  "file_paths": ["src/**/*.rs"],
  "create_todo": true
}
```

This will check files against the specified rules and automatically create a todo item for each violation found, making it easy to track and fix violations systematically.

