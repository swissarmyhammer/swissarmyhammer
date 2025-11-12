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
