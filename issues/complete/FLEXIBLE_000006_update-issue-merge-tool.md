# Update Issue Merge Tool for Source-Aware Merging

Refer to ./specification/flexible_base_branch_support.md

## Goal

Update the issue merge tool to retrieve source branch from issue metadata and merge back to the correct branch.

## Tasks

1. **Update Merge Tool Implementation**
   - Modify `MergeIssueTool::execute()` in `swissarmyhammer-tools/src/mcp/tools/issues/merge/mod.rs:58-143`
   - Retrieve source branch from issue metadata instead of assuming main
   - Pass source branch to git operations merge method
   - Update success messages to reference actual target branch

2. **Add Source Branch Retrieval**
   - Get source branch from issue data after loading issue
   - Handle backwards compatibility for issues without source branch (default to main)
   - Validate source branch exists before attempting merge

3. **Update Error Handling and Messages**  
   - Update success message from "merged to main" to use actual source branch
   - Add specific error handling for missing source branch
   - Integrate with abort tool for irrecoverable situations (deleted source branch)

## Implementation Details

- Location: `swissarmyhammer-tools/src/mcp/tools/issues/merge/mod.rs`
- Focus on the `execute()` method  
- Integration with updated git operations
- Error handling and user messaging

## Testing Requirements

- Test merge back to main branch (backwards compatibility)
- Test merge back to feature branch
- Test merge back to release branch  
- Test error handling when source branch is deleted
- Test backwards compatibility with issues that don't have source branch
- Test integration with abort tool for error conditions

## Success Criteria

- Issue branches merge back to their recorded source branch
- Backwards compatibility with existing issues (merge to main)
- Clear error messages when source branch is missing
- Success messages reference actual target branch
- Integration with abort tool for irrecoverable errors
- All merge functionality preserved

This step completes the merge workflow for flexible base branch support.