# Remove Main Branch Requirement from Git Operations

Refer to ./specification/flexible_base_branch_support.md

## Goal

Update git operations to allow issue branch creation from any non-issue branch, removing the hardcoded main branch requirement.

## Tasks

1. **Update Branch Validation Logic**
   - Modify `validate_branch_operation()` in `git.rs:147-166` to accept any non-issue source branch
   - Remove restriction requiring main branch for issue creation
   - Maintain restriction preventing issue branch creation from other issue branches

2. **Update Error Messages**
   - Change error messages from "main branch" to "base branch" or "source branch"  
   - Provide clearer context about which operations are allowed from which branches

3. **Preserve Backwards Compatibility**
   - Keep `main_branch()` method for tools that still need it
   - Ensure existing main/master workflows continue to work unchanged

## Implementation Details  

- Location: `swissarmyhammer/src/git.rs`
- Focus on `validate_branch_operation()` method
- Update error message strings to be branch-agnostic
- Maintain existing test compatibility

## Testing Requirements

- Test issue branch creation from feature branches
- Test issue branch creation from release branches  
- Test that issue-to-issue branch creation is still prevented
- Test backwards compatibility with main/master workflows
- Update existing tests that assume main branch requirement

## Success Criteria

- Can create issue branches from any non-issue branch
- Issue-to-issue branch creation is still prevented
- Error messages are clear and branch-agnostic
- All existing main/master workflows continue to work
- All tests pass

This step removes the core restriction that prevents flexible branching workflows.