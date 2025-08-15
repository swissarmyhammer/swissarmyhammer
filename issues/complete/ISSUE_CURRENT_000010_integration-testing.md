# Integration Testing and Workflow Validation

Refer to ./specification/issue_current.md

## Goal

Perform comprehensive integration testing to ensure all workflows continue to function correctly with the consolidated `issue_show` tool and updated builtin prompts.

## Tasks

1. **Test complete issue workflow**:
   - Create a test issue using `issue_create`
   - Switch to work on it using `issue_work`
   - Test `issue_show current` returns correct current issue
   - Test `issue_show next` functionality with multiple issues
   - Complete the workflow using updated prompts

2. **Test builtin prompt integration**:
   - Test each updated builtin prompt with new tool syntax
   - Verify prompt execution works end-to-end
   - Test prompts with various issue states and scenarios
   - Ensure prompt logic and behavior remain unchanged

3. **Test edge cases and error scenarios**:
   - Test behavior when not on an issue branch
   - Test behavior when no pending issues exist
   - Test behavior with git operations unavailable
   - Test handling of invalid parameters and edge cases

4. **Test CLI integration**:
   - Test CLI commands that use the updated prompts
   - Verify MCP tool integration works correctly
   - Test both direct tool calls and prompt-based usage
   - Ensure response formatting is consistent

5. **Performance and reliability testing**:
   - Test tool response times are acceptable
   - Test concurrent usage scenarios
   - Test rate limiting behavior
   - Test memory usage and resource cleanup

6. **Test backward compatibility**:
   - Ensure existing workflows that use regular `issue_show` work unchanged
   - Test with various issue name formats and edge cases
   - Verify response formats maintain consistency
   - Test error handling matches previous behavior

## Expected Outcome

Complete validation that:
- All issue management workflows work correctly
- Builtin prompts function identically to before
- Performance and reliability meet requirements
- No regression in existing functionality

## Success Criteria

- All integration tests pass successfully
- End-to-end workflows work correctly with updated tools
- Performance requirements are met
- No regressions in existing functionality
- User experience remains consistent and intuitive