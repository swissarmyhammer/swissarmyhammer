Check source code files against SwissArmyHammer rules for code quality and standards compliance.

## Examples

```json
{
  "rule_names": ["no-unwrap", "no-panic"],
  "file_paths": ["src/**/*.rs"]
}
```

## Returns

Returns list of violations with file path, line number, rule name, severity, and message. If no violations, returns success message.
