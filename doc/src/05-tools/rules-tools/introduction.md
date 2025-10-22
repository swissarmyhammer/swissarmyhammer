# Rules Operations

The Rules Operations tools provide automated code quality checking against defined standards, enabling consistent enforcement of coding practices across projects.

## Overview

Rules tools help maintain code quality by checking source files against defined standards. This enables:

- Automated code quality validation
- Consistent enforcement of best practices
- Early detection of potential issues
- Integration with CI/CD pipelines

## Key Concepts

### Rule Definitions

Rules are defined in configuration files that specify:

- **Pattern**: What to look for in source code
- **Severity**: How serious a violation is (error, warning, info, hint)
- **Category**: Logical grouping of related rules
- **Message**: Explanation shown when rule is violated

### Severity Levels

Rules are classified by severity:

- **Error**: Must be fixed before committing (blocks CI)
- **Warning**: Should be fixed soon (reported but doesn't block)
- **Info**: Informational messages for improvement
- **Hint**: Suggestions for best practices

### Scope Control

Rules can be applied to:

- **All files**: Check entire codebase
- **Specific paths**: Check only certain directories
- **Glob patterns**: Check files matching patterns (e.g., `**/*.rs`)
- **Changed files**: Check only files modified on current branch

## Available Tools

- [`rules_check`](check.md) - Check source code files against defined rules

## Use Cases

### Pre-commit Validation

```bash
# Check all changed files before commit
sah rules check --changed

# Only show errors
sah rules check --changed --severity error
```

### Continuous Integration

```yaml
# CI workflow checking specific rules
steps:
  - name: Code Quality Check
    run: sah rules check --severity error --max-errors 0
```

### Targeted Checks

```bash
# Check specific file types
sah rules check --file-paths "src/**/*.rs"

# Check against specific rules
sah rules check --rule-names no-unwrap no-panic
```

### Code Review Automation

Workflows can use rules_check to automatically review code quality:

```yaml
### quality_check
Check code quality for changed files
**Actions**:
  - tool: rules_check
    changed: true
    severity: error
**Next**: report
```

## Configuration

### Rule Files

Rules are defined in `.swissarmyhammer/rules/` directory:

```yaml
# .swissarmyhammer/rules/rust.yaml
rules:
  - name: no-unwrap
    pattern: '\\.unwrap\\('
    severity: error
    category: safety
    message: "Avoid using unwrap() - use proper error handling"

  - name: no-panic
    pattern: 'panic!\\('
    severity: warning
    category: reliability
    message: "Avoid panic! - return Result instead"
```

### Project Standards

Define project-specific standards in `CODING_STANDARDS.md`:

```markdown
## Error Handling

- Never use `.unwrap()` in production code
- Always propagate errors using `Result<T, E>`
- Use `expect()` only in tests with descriptive messages
```

## Integration

Rules tools integrate seamlessly with:

- **Workflows**: Use in action blocks for automated quality gates
- **MCP Protocol**: Available as `rules_check` tool in Claude Code
- **CLI**: Direct command-line access via `sah rules check`
- **Git Hooks**: Run as pre-commit or pre-push hooks

## Best Practices

### Start with Errors Only

When adopting rules, start by enforcing only error-level violations:

```bash
# Initial adoption - fix all errors first
sah rules check --severity error
```

Gradually increase strictness as the codebase improves.

### Use Changed Files Filter

For large codebases, check only changed files:

```bash
# Fast checks on modified files
sah rules check --changed --severity error
```

This provides fast feedback without checking entire codebase.

### Categorize Rules Logically

Group related rules by category for easier management:

- `safety` - Memory safety, undefined behavior
- `reliability` - Error handling, panic usage
- `style` - Code formatting, naming conventions
- `performance` - Known performance anti-patterns

### Document Exceptions

When a rule must be violated, document why:

```rust
// EXCEPTION: unwrap() is safe here because we just checked is_some()
let value = option.unwrap();
```

## Performance

The rules checker is optimized for speed:

- **Parallel Processing**: Checks multiple files concurrently
- **Incremental Checking**: Only checks specified files
- **Fast Pattern Matching**: Uses compiled regex patterns
- **Early Exit**: Stops at max_errors to fail fast

## Extending Rules

Add custom rules by creating rule definition files:

```yaml
# .swissarmyhammer/rules/custom.yaml
rules:
  - name: require-doc-comments
    pattern: '^pub\s+fn\s+\w+.*\n(?!\s*///)'
    severity: warning
    category: documentation
    message: "Public functions must have doc comments"
```

Rules are automatically loaded and applied on next check.
