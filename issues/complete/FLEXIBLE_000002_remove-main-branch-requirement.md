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

## Proposed Solution

After analyzing the codebase, I've identified the core issue in the `validate_branch_operation()` method in `swissarmyhammer/src/git.rs:147-166`. The current implementation has these restrictions:

1. **Current Logic**: Only allows issue branch creation/switching from main branch
2. **Issue Branch Pattern**: Branches following the `issue/{name}` pattern are considered issue branches
3. **Key Restriction**: Lines 154-164 prevent any operation when not on main branch

**My Implementation Plan**:

1. **Create helper function** to detect if a branch is an issue branch (using `issue/` prefix)
2. **Update validation logic** to:
   - Allow issue branch operations from any non-issue branch (not just main)
   - Continue preventing issue-to-issue branch operations
   - Update error messages to reference "source branch" instead of "main branch"
3. **Preserve existing behavior** for main/master workflows while enabling flexibility

**Key Changes**:
- Replace main branch requirement with "non-issue branch" requirement
- Update error messages to be branch-agnostic
- Maintain existing safety restrictions against issue-to-issue branching

This approach maintains backwards compatibility while enabling flexible branching workflows as specified in the requirements.

## Implementation Complete ✅

Successfully implemented the removal of main branch requirement from git operations. The changes have been completed and tested.

### Changes Made

1. **Added Helper Method** (`git.rs:111-114`):
   - Created `is_issue_branch()` method to detect issue branches using `issue/` prefix

2. **Updated Validation Logic** (`git.rs:152-171`):
   - Modified `validate_branch_operation()` to check if current branch is an issue branch
   - Removed hardcoded main branch requirement
   - Now allows issue branch creation from any non-issue branch

3. **Updated Error Messages**:
   - Changed from "Must be on main branch" to "Must be on a non-issue branch"
   - Updated from "Please switch to main first" to "Please switch to a non-issue branch first"

4. **Updated Documentation**:
   - Updated method comments to reflect new flexible branching rules

5. **Added Test Coverage** (`git.rs:803-825`):
   - Added `test_create_work_branch_from_feature_branch_succeeds()` to verify new functionality

### Test Results

All 19 git tests pass, including:
- ✅ `test_create_work_branch_from_issue_branch_should_abort` - Issue-to-issue prevention still works
- ✅ `test_switch_to_existing_issue_branch_from_issue_branch_should_abort` - Issue-to-issue switching prevention
- ✅ `test_create_work_branch_from_main_succeeds` - Main branch workflow still works (backwards compatibility)
- ✅ `test_create_work_branch_from_feature_branch_succeeds` - New functionality verified

### Success Criteria Met

- ✅ Can create issue branches from any non-issue branch
- ✅ Issue-to-issue branch creation is still prevented
- ✅ Error messages are clear and branch-agnostic
- ✅ All existing main/master workflows continue to work
- ✅ All tests pass

The core restriction has been successfully removed, enabling flexible branching workflows while maintaining safety guarantees.