# Exclude Partials from Rule Checking

## Description
Partials should not be treated as rules in the rule checker. They are meant to be included/imported by other rules, not executed as standalone rules.

## Evidence
From the error output:
```
✓ All 16 rules are valid
Rule '_partials/code-block' violated in /Users/wballard/github/sah/swissarmyhammer-rules/tests/rule_library_integration_test.rs
```

The `_partials/code-block` partial is being checked as if it were a rule, and likely counted in the "16 rules" count.

## Problem
- Partials are being loaded and executed as rules during `rule check`
- Partials are included in rule counts (validation and checking)
- This causes incorrect violations and confusing error messages
- Partials are designed to be reusable components, not standalone rules

## Implementation Guidelines

### Identifying Partials
- **DO NOT** hard code assumptions about `_partials` directory names
- **DO** identify partials by detecting `{% partial %}` tag in the file content
- Look at how the `list` command filters out partials for reference

### Preserving Rendering
- **DO NOT** change the virtual file system
- **DO NOT** break partial rendering - partials must still be available when rendering rules that include them
- Partials should be loaded into the VFS but excluded from rule checking/validation

### Testing Requirements
- **MUST** have unit tests for rendering a rule with a partial
- Ensure existing partial rendering continues to work
- Test that partials are excluded from rule checking but still available for inclusion

## Expected Behavior
- Files containing `{% partial %}` are excluded from rule checking
- Partials are not counted in any rule counts
- Only actual rules should be validated and checked
- Partials remain available in VFS for rendering when included by other rules
- Validation output shows "All rules valid" instead of "All X rules are valid"

## Acceptance Criteria
- [ ] Rule checker filters out files containing `{% partial %}` tag
- [ ] Partials are not validated or executed as standalone rules
- [ ] Partials are not included in any rule counts
- [ ] Validation output shows "All rules valid" without counts
- [ ] Partials still work when included in other rules (rendering not broken)
- [ ] Virtual file system is not modified
- [ ] Unit tests verify rendering a rule with a partial still works
- [ ] Implementation matches filtering approach used in `list` command
