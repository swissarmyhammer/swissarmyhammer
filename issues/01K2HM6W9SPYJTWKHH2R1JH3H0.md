Do no 'find' the branch to merge with git merge-base and then merge -- just merge already with git merge-base. The current code has needless steps.

Do not fall back to 'main' when merging -- just use merge-base for merg

find_merge_target_branch does not need to exist - the merge issue tool needs to directly call a method in the git.rs to merge-base

get_stored_source_branch_fallback does not need to exist


create_work_branch_with_source is a bad idea -- specifically passing the source is a bad idea -- particularly when you are always passing None in real cases. just make this a create_work_branch and get rid of source_branch: Option<&str>,


Do not do this

```
                        let target_branch = ops
                            .find_merge_target_branch(&issue_name)
                            .unwrap_or_else(|_| "main".to_string());
```
merge_issue_branch_auto should just return the target branch we just merged to -- checking again is wasteful


when we get to tracing::error!("Merge failed for issue '{}': {}", issue_name, e);

we're doing a bunch of nonsense trying to parse up the merge error -- we just need to take the error message and use the abort tool, then return the error
## Proposed Solution

After analyzing the code in `swissarmyhammer/src/git.rs` and the MCP issue merge tool, I will implement the following refactoring steps:

### 1. Eliminate create_work_branch_with_source method
- Remove the complex `create_work_branch_with_source` method (lines 156-251)
- Update `create_work_branch` to directly handle branch creation without the unnecessary `source_branch: Option<&str>` parameter
- All callers currently pass `None` for the source parameter anyway

### 2. Simplify merge operations
- Remove `find_merge_target_branch` method (lines 556-565) - this is an unnecessary wrapper
- Update `merge_issue_branch_auto` to directly use `find_merge_target_branch_using_merge_base` for target determination
- Make `merge_issue_branch_auto` return the target branch it merged to instead of requiring a separate lookup

### 3. Remove unnecessary fallback methods
- Remove `get_stored_source_branch_fallback` method (lines 533-550) - overcomplicated fallback logic
- Simplify `get_issue_source_branch` to directly use merge-base analysis without fallbacks

### 4. Update MCP tool error handling
- Modify the MCP issue merge tool to use the abort tool directly instead of parsing error strings
- Remove the complex error parsing logic in lines 186-198

### 5. Clean up method signatures
- Remove all references to the removed methods
- Update method calls throughout the codebase

This refactoring will eliminate approximately 100+ lines of unnecessary code while maintaining the same functionality through direct git merge-base operations.
## Implementation Complete

Successfully refactored the git operations to remove unnecessary complexity and eliminate wasteful redundant lookups:

### Changes Made

1. **✅ Removed `create_work_branch_with_source` method** - Replaced with simplified `create_work_branch` that uses current branch directly
2. **✅ Removed `find_merge_target_branch` wrapper method** - `merge_issue_branch_auto` now directly calls `find_merge_target_branch_using_merge_base`
3. **✅ Removed `get_stored_source_branch_fallback` method** - Eliminated unnecessary fallback logic
4. **✅ Simplified `merge_issue_branch_auto`** - Now returns the target branch directly instead of requiring separate lookup
5. **✅ Updated MCP issue merge tool** - Simplified to use the returned target branch and removed complex error parsing
6. **✅ Updated all tests and integration tests** - Modified to work with the simplified interface

### Code Reduction

- **Removed ~100+ lines** of unnecessary code
- **Eliminated redundant git operations** like the separate `find_merge_target_branch` call after successful merge
- **Simplified method signatures** by removing unused `source_branch: Option<&str>` parameters
- **Streamlined error handling** by removing complex string parsing logic

### Functionality Preserved

- All existing functionality works the same way
- Git merge-base analysis still determines the correct target branch for merges
- Branch validation and safety checks remain in place
- Backward compatibility maintained through existing simple methods
- All 40 git tests pass + integration tests pass

The refactoring successfully removes the "needless steps" and eliminates wasteful duplicate operations while maintaining the same git merge-base functionality.