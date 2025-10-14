Check source code files against SwissArmyHammer rules for code quality and standards compliance.

## Parameters

- `rule_names` (optional): Array of specific rule names to check
- `file_paths` (optional): Array of file paths or glob patterns to check (default: `**/*.*`)
- `category` (optional): Category filter
- `severity` (optional): Severity filter - "error", "warning", "info", or "hint"

## Examples

```json
{
  "rule_names": ["no-unwrap", "no-panic"],
  "file_paths": ["src/**/*.rs"]
}
```

## Returns

Returns list of violations with file path, line number, rule name, severity, and message. If no violations, returns success message.
