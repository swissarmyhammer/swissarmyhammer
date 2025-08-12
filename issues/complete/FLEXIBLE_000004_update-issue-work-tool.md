# Update Issue Work Tool for Flexible Base Branches

Refer to ./specification/flexible_base_branch_support.md

## Goal

Update the issue work tool to use flexible base branch support and store source branch information.

## Tasks

1. **Update Work Tool Implementation**
   - Modify `WorkIssueTool::execute()` in `swissarmyhammer-tools/src/mcp/tools/issues/work/mod.rs:48-78`
   - Get current branch before switching to issue branch
   - Pass current branch as source to git operations
   - Store source branch in issue metadata when creating new issue branch

2. **Integrate with Source Branch Tracking**
   - Use updated `create_work_branch()` method with source branch parameter
   - Ensure source branch is captured and stored in issue data
   - Handle case where issue already exists with different source branch

3. **Add Source Branch Validation**
   - Prevent working on issue from another issue branch
   - Provide clear error messages for invalid source branch operations
   - Handle edge cases gracefully

## Implementation Details

- Location: `swissarmyhammer-tools/src/mcp/tools/issues/work/mod.rs`
- Focus on the `execute()` method
- Integration with git operations and issue storage
- Error handling and validation

## Testing Requirements  

- Test working on issue from main branch
- Test working on issue from feature branch
- Test working on issue from release branch
- Test prevention of issue-to-issue work operations
- Test error handling for invalid operations
- Test that source branch is correctly stored in issue metadata

## Success Criteria

- Can start working on issues from any non-issue branch
- Source branch is correctly captured and stored
- Issue-to-issue work operations are prevented  
- Clear error messages for invalid operations
- Integration with flexible git operations works correctly

This step connects the issue management layer with the flexible git operations to enable flexible branching workflows.

## Proposed Solution

After analyzing the current implementation, I've identified that the WorkIssueTool needs to be updated to:

1. **Capture Current Branch Before Switching**: Get the current branch before attempting to create the work branch to ensure the source branch information is properly available for new issues.

2. **Handle New Issue Creation**: When working on a new issue (doesn't exist yet), the tool should capture the current branch and store it as the source branch in the issue metadata.

3. **Source Branch Validation**: Prevent creating issue branches from other issue branches by validating the current branch before proceeding.

4. **Integration with Issue Storage**: Ensure that when a new issue is created via the work tool, it gets the proper source branch information stored.

### Implementation Steps:

1. Modify `WorkIssueTool::execute()` in `/Users/wballard/github/swissarmyhammer/swissarmyhammer-tools/src/mcp/tools/issues/work/mod.rs`
2. Get current branch before any git operations
3. For existing issues, use stored source branch (current behavior)
4. For new issues, capture current branch and create issue with source branch information
5. Add validation to prevent issue-to-issue work operations
6. Update error handling to provide clear messages

This approach will enable flexible base branch support while maintaining backward compatibility.