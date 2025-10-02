# Add testing and implementation for rule partials

## Problem
There is no testing of partials, and no partials are defined in the builtin rules.

## Background
Partials are reusable template components that can be included in rule templates. They enable:
- Code reuse across multiple rules
- Consistent formatting and structure
- Easier maintenance of common patterns

## Current State
- No partial templates exist in the builtin rules
- No tests verify partial functionality
- Unclear if partial system works correctly

## Tasks
1. **Define useful partials** for common rule patterns:
   - Common severity explanations
   - Standard output formats
   - Shared violation descriptions
   - Common remediation steps

2. **Add tests** for partial functionality:
   - Test partial resolution
   - Test partial rendering within rules
   - Test partial not found error handling
   - Test nested partials (if supported)

3. **Update existing rules** to use partials where appropriate
   - Identify duplicated content across rules
   - Extract to partials
   - Update rules to use partials

## Benefits
- Reduces duplication across rules
- Makes rules easier to maintain
- Ensures consistent output formatting
- Validates that the partial system works correctly
