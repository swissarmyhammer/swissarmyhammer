Check source code files against SwissArmyHammer rules for code quality and standards compliance.

## Parameters

- `rule_names` (optional): Array of specific rule names to check
- `severity` (optional): Filter by severity level (error, warning, info, hint)
- `category` (optional): Filter rules by category
- `file_paths` (optional): Array of file paths or glob patterns to check (defaults to `**/*.*`)
- `max_errors` (optional): Maximum number of ERROR violations to return
- `changed` (optional): Check only changed files (intersects with file_paths if provided)
- `create_todo` (optional): Automatically create a todo item for each rule violation found (default: false)

## Examples

Basic rule check:
```json
{
  "rule_names": ["no-unwrap", "no-panic"],
  "file_paths": ["src/**/*.rs"]
}
```

Check with automatic todo creation:
```json
{
  "rule_names": ["code-quality/cognitive-complexity"],
  "file_paths": ["src/**/*.rs"],
  "create_todo": true
}
```

Check changed files and create todos:
```json
{
  "changed": true,
  "create_todo": true
}
```

## Todo Format

When `create_todo` is enabled, each violation creates a todo with:

**Task field:**
```
Fix [rule_name] violation in [file_path]
```

**Context field:**
```markdown
## Rule Violation

**Rule**: [rule_name]
**File**: [file_path]
**Severity**: [severity]

## Violation Details

[violation message from LLM]

## How to Fix

See rule documentation for guidance on resolving this violation.
```

## Returns

Returns list of violations with file path, line number, rule name, severity, and message. If no violations, returns success message.

When `create_todo` is enabled, the completion notification includes the number of todos created.
