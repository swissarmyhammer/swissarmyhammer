# Add Comprehensive Tests for Enhanced issue_show

Refer to ./specification/issue_current.md

## Goal

Create comprehensive test coverage for the enhanced `issue_show` tool with special parameter handling, ensuring all functionality works correctly.

## Tasks

1. **Test "current" parameter functionality**:
   - Test `issue_show current` when on issue branch
   - Test `issue_show current` when not on issue branch  
   - Test `issue_show current` when git operations unavailable
   - Test branch name parsing edge cases
   - Test config integration for branch prefix

2. **Test "next" parameter functionality**:
   - Test `issue_show next` when pending issues exist
   - Test `issue_show next` when no pending issues exist
   - Test `issue_show next` with multiple pending issues (alphabetical order)
   - Test storage backend error handling
   - Test async operation handling

3. **Test backward compatibility**:
   - Ensure all existing `issue_show` tests still pass
   - Test regular issue names work exactly as before
   - Test response formatting consistency
   - Test error handling for invalid issue names

4. **Test parameter validation**:
   - Test empty name parameter
   - Test special parameter case sensitivity
   - Test invalid special parameter values
   - Test parameter type validation

5. **Test integration scenarios**:
   - Test switching between regular and special parameters
   - Test concurrent access scenarios
   - Test rate limiting behavior
   - Test logging and tracing output

6. **Update existing test files**:
   - Update any tests that use `issue_current` or `issue_next` directly
   - Ensure test coverage meets project standards
   - Follow established testing patterns

## Expected Outcome

Complete test coverage ensuring:
- All new functionality works correctly
- Backward compatibility is maintained
- Error cases are handled properly
- Performance characteristics are acceptable

## Success Criteria

- All tests pass consistently
- Test coverage includes edge cases and error conditions
- Tests follow established patterns in codebase
- Both unit and integration tests are included
- Performance requirements are met