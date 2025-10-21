# Add max_errors Parameter to Rule Check Tool

## Summary
Add a new optional `max_errors` parameter to the rule check tool and CLI to enable early termination and chunked error processing.

## Requirements

### Parameter Specification
- **Name**: `max_errors`
- **Type**: Optional integer
- **Default**: 1
- **Purpose**: Limit the number of errors returned by the rule checker

### Implementation Details

1. **CLI Switch**: Add `--max-errors` flag to the rules check CLI command
   - Should accept an integer value
   - Default to 1 if not specified
   - Example: `sah rules check --max-errors 5 src/**/*.rs`

2. **Tool Interface**: Add `max_errors` as an optional parameter to the `rules_check` tool

3. **Rule Checker Integration**: Pass `max_errors` through to the underlying rule checker options

4. **Early Termination**: The checker should abort once it reaches the `max_errors` threshold, avoiding unnecessary processing of remaining files

### Benefits
- Faster feedback when checking large codebases
- Ability to work on errors incrementally in chunks
- Reduced processing time when only a sample of errors is needed
- Better user experience for iterative fixing workflows

### Example Usage

**CLI:**
```bash
sah rules check --max-errors 5 --rule no-unwrap src/**/*.rs
```

**MCP Tool:**
```json
{
  "rule_names": ["no-unwrap"],
  "file_paths": ["src/**/*.rs"],
  "max_errors": 5
}
```

This would return up to 5 errors and then abort the check.