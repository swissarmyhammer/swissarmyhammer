# Update Git Operations for Source-Aware Branch Creation  

Refer to ./specification/flexible_base_branch_support.md

## Goal

Modify git operations to track and use source branch information when creating issue branches.

## Tasks

1. **Update create_work_branch Method**
   - Add optional `source_branch` parameter to `create_work_branch()` in `git.rs:120-142`
   - When no source provided, use current branch as source
   - Validate source branch exists and is not an issue branch
   - Return source branch information along with created branch name

2. **Enhance Branch Creation Logic**  
   - Track which branch the issue branch was created from
   - Ensure issue branch creation fails if source branch is invalid
   - Maintain existing resume behavior (already on target branch)

3. **Update Method Signatures**
   - Modify `create_work_branch(&self, issue_name: &str)` to optionally accept source branch
   - Update return type to include source branch information if needed
   - Maintain backwards compatibility with existing callers

## Implementation Details

- Location: `swissarmyhammer/src/git.rs`  
- Focus on `create_work_branch()` method around lines 120-142
- Add proper validation for source branch parameter
- Maintain existing error handling patterns

## Testing Requirements

- Test branch creation with explicit source branch parameter
- Test branch creation with implicit source branch (current branch)
- Test validation of invalid source branches
- Test resume behavior with source branch tracking
- Test backwards compatibility with existing callers

## Success Criteria

- `create_work_branch()` accepts optional source branch parameter
- Source branch validation prevents invalid operations
- Issue branches are created from specified or current source branch
- Resume behavior works correctly with source branch tracking
- All existing functionality preserved

This step enables the git operations layer to handle flexible source branches while maintaining backwards compatibility.