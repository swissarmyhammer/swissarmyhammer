# Implement Environment Variable Substitution in Template Integration

## Location
`swissarmyhammer/src/sah_config/template_integration.rs:52`

## Current State
There's a TODO comment indicating that environment variable substitution needs to be implemented. The code currently merges configuration values and workflow template variables but doesn't handle environment variable references.

## Requirements
Implement environment variable substitution that:
- Processes values like `${VAR_NAME:-default}` in configuration values
- Replaces them with actual environment variable values
- Supports default values when environment variable is not set
- Maintains proper priority order: env vars > config values, but workflow template vars override all

## Implementation Details
1. Parse configuration values for patterns like `${VAR_NAME}` or `${VAR_NAME:-default}`
2. Look up the environment variable using `std::env::var()`
3. If found, replace with the value
4. If not found and default provided, use the default
5. If not found and no default, either leave as-is or error (decide on behavior)
6. This should happen after loading config values but before workflow template vars are applied

## Testing Requirements
- Test basic variable substitution
- Test with default values
- Test missing variables without defaults
- Test nested/complex patterns
- Test that workflow vars still override env-substituted values

## Acceptance Criteria
- [ ] Environment variable patterns are detected and parsed
- [ ] Variables are properly substituted with env values
- [ ] Default values work when env var is not set
- [ ] Priority order is maintained correctly
- [ ] All tests pass
- [ ] No performance regression