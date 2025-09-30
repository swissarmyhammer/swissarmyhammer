# Implement Rule Validate Command

Refer to ideas/rules.md

## Goal

Implement `sah rule validate` command to validate all rules before checking.

## Context

Validation ensures rules are well-formed before attempting to check files. This catches configuration errors early.

## Implementation

1. In `validate.rs`, implement `execute_validate_command()`:
   - Load all rules via RuleResolver
   - Call `rule.validate()` on each rule
   - Collect validation errors
   - Display results:
     - ✓ Valid rules (count)
     - ✗ Invalid rules with details
   - Exit with code 1 if any invalid

2. Validation checks:
   - Required fields present (title, description, severity)
   - Severity is valid value
   - Template syntax valid (liquid)
   - No parameters field (rules don't have parameters)

3. Display validation issues clearly with file paths

## Testing

- Test with all valid rules
- Test with invalid rules (missing fields)
- Test with syntax errors
- Test exit codes

## Success Criteria

- [ ] Validate command implemented
- [ ] Validates all rules
- [ ] Clear error messages
- [ ] Proper exit codes
- [ ] Tests passing
