# Update Git Operations for Source-Aware Merging

Refer to ./specification/flexible_base_branch_support.md

## Goal

Update git operations to merge issue branches back to their source branch instead of always merging to main.

## Tasks

1. **Update merge_issue_branch Method**
   - Modify `merge_issue_branch()` in `git.rs:206-271`
   - Accept source branch parameter instead of hardcoding main branch
   - Validate source branch exists before attempting merge
   - Switch to source branch instead of main branch before merging

2. **Add Source Branch Validation**
   - Check that source branch still exists before merge
   - Provide clear error if source branch was deleted
   - Handle edge cases where source branch has diverged significantly

3. **Update Merge Logic**
   - Replace `let main_branch = self.main_branch()?;` with source branch parameter
   - Update checkout operation to use source branch
   - Update merge commit message to reference source branch
   - Maintain existing conflict detection and error handling

## Implementation Details

- Location: `swissarmyhammer/src/git.rs`
- Focus on `merge_issue_branch()` method around lines 206-271
- Change method signature to accept source branch parameter
- Update internal logic to use source branch instead of main

## Testing Requirements

- Test merge to main branch (backwards compatibility)
- Test merge to feature branch  
- Test merge to release branch
- Test error handling when source branch doesn't exist
- Test merge conflict handling with source branch
- Update existing tests to pass source branch parameter

## Success Criteria

- `merge_issue_branch()` accepts source branch parameter
- Issue branches merge to their source branch instead of main
- Proper validation and error handling for missing source branches
- Merge conflicts are handled correctly with source branch
- All existing merge functionality preserved
- Backwards compatibility maintained

This step enables the core merge functionality to work with flexible base branches.