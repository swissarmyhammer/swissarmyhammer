# Enhanced Error Handling and Edge Cases for Flexible Branching

Refer to ./specification/flexible_base_branch_support.md

## Goal

Add comprehensive error handling for edge cases in flexible base branch workflows, including integration with the abort tool.

## Tasks

1. **Abort Tool Integration**
   - Integrate abort tool when source branch is deleted before merge
   - Integrate abort tool when merge conflicts cannot be resolved automatically  
   - Add clear abort messages explaining the issue and next steps

2. **Enhanced Edge Case Handling**
   - Handle case where source branch is deleted after issue creation
   - Handle case where source branch has diverged significantly from issue branch
   - Prevent circular issue branch creation scenarios
   - Add validation for complex branching scenarios

3. **Improved Error Messages**
   - Provide context about source branch in all error messages
   - Suggest recovery actions for common error scenarios
   - Include branch information in validation error messages
   - Make error messages consistent across git operations and MCP tools

4. **Source Branch Validation**
   - Add comprehensive validation that source branch exists and is accessible
   - Validate source branch is not corrupted or in invalid state
   - Check that user has permissions to merge to source branch

## Implementation Details

- Location: Both `swissarmyhammer/src/git.rs` and `swissarmyhammer-tools/src/mcp/tools/issues/`
- Add abort tool integration points in merge operations
- Enhance existing error handling with source branch context
- Add new validation methods for complex scenarios

## Testing Requirements

- Test abort tool integration when source branch is deleted
- Test abort tool integration with merge conflicts
- Test error handling for corrupted source branch
- Test validation prevents circular issue branch creation
- Test error messages provide helpful context and recovery suggestions
- Test edge cases with complex branching scenarios

## Success Criteria

- Abort tool integration works correctly for irrecoverable scenarios
- Error messages provide clear context about source branch issues
- Edge cases are handled gracefully without data loss
- Recovery suggestions help users understand next steps  
- Complex branching scenarios are validated and prevented
- No circular dependencies or invalid states possible

This step ensures robust error handling for all flexible branching scenarios.