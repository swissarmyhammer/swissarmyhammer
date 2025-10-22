# rules_check

Check source code files against SwissArmyHammer rules for code quality and standards compliance.

## Purpose

The `rules_check` tool automates code quality validation by checking source files against defined rules. This enables:

- Consistent enforcement of coding standards
- Early detection of potential issues
- Automated code review checks
- Integration with development workflows

## Parameters

### rule_names (optional)

Array of specific rule names to check. When provided, only these rules are evaluated.

- **Type**: Array of strings
- **Required**: No
- **Example**: `["no-unwrap", "no-panic", "require-error-handling"]`
- **Default**: All rules are checked

### file_paths (optional)

Array of file paths or glob patterns to check. Supports standard glob syntax.

- **Type**: Array of strings
- **Required**: No
- **Example**: `["src/**/*.rs", "tests/**/*.rs"]`
- **Default**: `**/*.*` (all files)

### changed (optional)

If true, check only files that have changed on the current git branch.

- **Type**: Boolean
- **Required**: No
- **Default**: `false`

When combined with `file_paths`, the intersection is checked (files that match the patterns AND have changed).

### category (optional)

Filter rules by category. Only rules in this category are evaluated.

- **Type**: String
- **Required**: No
- **Example**: `"safety"`, `"reliability"`, `"style"`
- **Default**: All categories

### severity (optional)

Filter rules by minimum severity level. Only rules at or above this severity are checked.

- **Type**: String (enum)
- **Required**: No
- **Values**: `"error"`, `"warning"`, `"info"`, `"hint"`
- **Default**: All severities

### max_errors (optional)

Maximum number of ERROR-severity violations to return. Useful for failing fast in CI.

- **Type**: Integer
- **Required**: No
- **Minimum**: 1
- **Default**: Unlimited

## Response Format

### With Violations

```json
{
  "violations": [
    {
      "file_path": "src/lib.rs",
      "line_number": 42,
      "rule_name": "no-unwrap",
      "severity": "error",
      "message": "Avoid using unwrap() - use proper error handling",
      "category": "safety"
    },
    {
      "file_path": "src/utils.rs",
      "line_number": 17,
      "rule_name": "no-panic",
      "severity": "warning",
      "message": "Avoid panic! - return Result instead",
      "category": "reliability"
    }
  ],
  "summary": {
    "total_violations": 2,
    "by_severity": {
      "error": 1,
      "warning": 1,
      "info": 0,
      "hint": 0
    },
    "files_checked": 156,
    "execution_time_ms": 342
  }
}
```

### No Violations

```json
{
  "message": "No violations found",
  "summary": {
    "total_violations": 0,
    "files_checked": 156,
    "execution_time_ms": 278
  }
}
```

## Examples

### MCP Usage (Claude Code)

```json
{
  "tool": "rules_check",
  "parameters": {
    "file_paths": ["src/**/*.rs"],
    "severity": "error"
  }
}
```

### CLI Usage

```bash
# Check all files
sah rules check

# Check only errors in Rust files
sah rules check --file-paths "**/*.rs" --severity error

# Check changed files only
sah rules check --changed

# Check specific rules
sah rules check --rule-names no-unwrap no-panic

# Fail fast after 10 errors
sah rules check --max-errors 10 --severity error

# Check by category
sah rules check --category safety --severity error
```

### Workflow Usage

```yaml
### quality_gate
Enforce code quality standards
**Actions**:
  - tool: rules_check
    changed: true
    severity: error
    max_errors: 0
**Next**: complete on success, fix_violations on error
```

## Use Cases

### Pre-commit Hook

```bash
#!/bin/bash
# .git/hooks/pre-commit

# Check only changed files for errors
violations=$(sah rules check --changed --severity error)

if [ $? -ne 0 ]; then
  echo "❌ Code quality check failed:"
  echo "$violations"
  exit 1
fi

echo "✅ Code quality check passed"
```

### CI/CD Pipeline

```yaml
# .github/workflows/quality.yml
name: Code Quality

on: [pull_request]

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2

      - name: Install SwissArmyHammer
        run: cargo install swissarmyhammer

      - name: Check Code Quality
        run: |
          sah rules check \
            --changed \
            --severity error \
            --max-errors 0
```

### Selective Checking

```bash
# Check only safety-critical code
sah rules check \
  --file-paths "src/core/**/*.rs" \
  --category safety \
  --severity error

# Check documentation quality
sah rules check \
  --file-paths "**/*.md" \
  --category documentation \
  --severity warning
```

### Code Review Automation

```bash
# Get violations for PR review
violations=$(sah rules check --changed --format json)

# Post as PR comment
echo "$violations" | jq -r '.violations[] | "- \(.file_path):\(.line_number): \(.message)"' | \
  gh pr comment --body-file -
```

## Exit Codes

The tool returns specific exit codes for scripting:

- **0**: No violations found
- **1**: Violations found (any severity)
- **2**: ERROR-severity violations found
- **3**: Tool execution error (invalid parameters, etc.)

## Performance Optimization

### Parallel Processing

The tool checks files in parallel for speed:

```bash
# Checks ~1000 files/second on modern hardware
time sah rules check --file-paths "**/*.rs"
```

### Incremental Checks

For large codebases, use `--changed` for fast feedback:

```bash
# Fast: Only checks modified files
sah rules check --changed

# Slower: Checks entire codebase
sah rules check
```

### Early Exit

Use `--max-errors` to stop after finding N errors:

```bash
# Stop after first error (fastest failure)
sah rules check --max-errors 1 --severity error
```

## Rule Definition Format

Rules are defined in YAML files under `.swissarmyhammer/rules/`:

```yaml
# .swissarmyhammer/rules/rust.yaml
rules:
  - name: no-unwrap
    pattern: '\\.unwrap\\('
    severity: error
    category: safety
    message: "Avoid using unwrap() - use proper error handling instead"

  - name: todo-comments
    pattern: '//\s*TODO:'
    severity: info
    category: maintenance
    message: "TODO comment found - consider creating an issue"
```

### Rule Fields

- `name`: Unique identifier for the rule
- `pattern`: Regular expression pattern to match
- `severity`: Violation severity (error/warning/info/hint)
- `category`: Logical grouping for the rule
- `message`: Human-readable explanation of the violation

## Best Practices

### Start Strict, Relax as Needed

Begin with error-only checks:

```bash
# Initial setup
sah rules check --severity error
```

Add warnings and info as the team adapts.

### Use Changed Files in Development

For fast feedback during development:

```bash
# Quick check before commit
sah rules check --changed --severity error
```

### Full Checks in CI

Run comprehensive checks in CI:

```bash
# CI pipeline
sah rules check --severity warning --max-errors 0
```

### Document Rule Exceptions

When code must violate a rule, document why:

```rust
// EXCEPTION to no-unwrap rule:
// This is safe because we verified is_some() above
let value = option.unwrap();
```

### Combine with Other Tools

Rules checking complements other tools:

```bash
# Complete quality check
cargo fmt --check && \
cargo clippy && \
sah rules check --severity error
```

## Error Handling

The tool handles various error conditions:

### Invalid Rule Names

```json
{
  "error": "Unknown rule: 'invalid-rule-name'",
  "available_rules": ["no-unwrap", "no-panic", "..."]
}
```

### Invalid File Patterns

```json
{
  "error": "Invalid glob pattern: '[invalid'",
  "pattern": "[invalid"
}
```

### No Rules Configured

```json
{
  "error": "No rules found. Create rule definitions in .swissarmyhammer/rules/",
  "hint": "See documentation for rule definition format"
}
```

## Limitations

### Pattern Matching Only

The tool uses regex pattern matching, not semantic analysis. It may:

- Miss violations in complex code structures
- Produce false positives for valid code
- Not understand language-specific contexts

For deeper analysis, combine with language-specific linters like `clippy` (Rust), `pylint` (Python), etc.

### Single-line Patterns

Rules match single lines. Multi-line violations require multiple rules or external tools.

### No Auto-fix

The tool identifies violations but does not automatically fix them. Use language-specific formatters and refactoring tools for fixes.
