Check source code files against SwissArmyHammer rules for code quality and standards compliance.

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

When `create_todo` is enabled, each violation creates a todo.
