# Rule Check Tool

Check source code files against SwissArmyHammer rules for code quality and standards compliance.

## Purpose

The rule check tool enables AI agents and external tools to validate code against defined rules through the MCP interface. This tool:
- Validates code against project-specific and built-in rules
- Identifies code quality issues, security concerns, and style violations
- Supports filtering by rule name and file patterns
- Provides detailed violation reports with severity levels

## Parameters

- `rule_names` (optional): Array of specific rule names to check
  - Type: array of strings
  - Description: Limit checking to specific rules (e.g., ["no-unwrap", "no-panic"])
  - Default: All available rules
  
- `file_paths` (optional): Array of file paths or glob patterns to check
  - Type: array of strings
  - Description: Files or patterns to check (e.g., ["src/**/*.rs", "lib/*.js"])
  - Default: `**/*.*` (all files respecting .gitignore)

## Response Format

### No Violations Found
```
✅ No rule violations found

Checked 5 rules against 23 files in 1234ms
```

### Violations Found
```
Found 2 violation(s) in 2 files (1234ms)

❌ no-unwrap [error] in src/main.rs:42
   Found unwrap() call which can panic. Use proper error handling instead.

❌ no-commented-code [info] in src/lib.rs:15
   Detected commented-out code block. Remove or uncomment if still needed.
```

## Use Cases

### Check All Rules
Validate all files against all rules:
```json
{}
```

### Check Specific Rules
Focus on security rules only:
```json
{
  "rule_names": ["no-hardcoded-secrets", "no-sql-injection"]
}
```

### Check Specific Files
Check only Rust files in src directory:
```json
{
  "file_paths": ["src/**/*.rs"]
}
```

### Combined Filtering
Check specific rules against specific files:
```json
{
  "rule_names": ["no-unwrap", "no-panic"],
  "file_paths": ["src/**/*.rs", "tests/**/*.rs"]
}
```

## Rule System

### Rule Sources
Rules are loaded from multiple sources with hierarchical precedence:
- Built-in rules (lowest precedence) - Embedded in the binary
- User rules (medium precedence) - `~/.swissarmyhammer/rules/*.md`
- Project rules (highest precedence) - `./rules/*.md` in your project

Higher precedence rules override lower ones by name.

### Rule Severity Levels
- **error**: Critical issues that should block code review/merge
- **warning**: Important issues that should be addressed
- **info**: Suggestions and style improvements
- **hint**: Minor suggestions and best practices

## Implementation Details

This tool is a CLI wrapper that invokes the `sah rule check` command. The wrapper:
- Constructs the appropriate CLI command with filters
- Executes the command asynchronously
- Parses the JSON output
- Returns structured results through MCP

The CLI-based approach avoids circular dependencies in the crate structure while leveraging the existing, well-tested rule checking functionality.

## Error Handling

The tool handles various error conditions:
- Invalid rule names are reported by the CLI
- Invalid glob patterns are caught and reported
- File access errors are surfaced with context
- Command execution failures are logged and returned as errors

## Performance Considerations

- Rule checking is performed by an AI agent, so execution time varies
- Large codebases may take several seconds to check
- Use file_paths filtering to focus on changed files for faster feedback
- Use rule_names filtering to check only relevant rules
