# Remove Main Branch Requirement and Support Flexible Base Branches

## Problem

Currently, the codebase has hardcoded assumptions about the "main" branch and requires all issue branches to be created from and merged back to the main branch. This prevents the use of feature branches and other development workflows where issues might need to branch from and merge back to different base branches.

## Current Limitations

Based on code analysis, the following locations explicitly reference or require the main branch:

### Core Git Operations (`swissarmyhammer/src/git.rs`)
- `main_branch()` method hardcoded to detect "main" or "master":80-83
- Branch validation requiring operations from main branch:117,123,131,146-154  
- Issue merge operations that always merge to main:202,205,228-229

### Documentation and Help Text
- CLI help text mentioning merging to main:254
- Built-in prompt descriptions referencing main branch merge
- MCP tool descriptions that reference main branch

### Tests
- Multiple test files that verify main branch operations
- Integration tests that check main branch functionality

## Proposed Solution

### 1. Remove Main Branch Requirements

**Allow issue creation from any non-issue branch:**
- Remove the restriction that requires being on main branch to create issues
- Allow creating issues from feature branches, development branches, etc.
- Maintain the restriction that prevents creating an issue branch from another issue branch

**Track the source branch:**
- When creating an issue branch, record which branch it was created from
- Store this information so we can merge back to the correct branch later

### 2. Flexible Base Branch Support

**Branch creation logic:**
```rust
// Current (restrictive):
// - Must be on main branch to create issue
// - Always merge back to main

// Proposed (flexible):
// - Can create issue from any non-issue branch
// - Merge back to the branch it was created from
```

**Base branch detection:**
- Replace hardcoded main branch detection with flexible base branch tracking
- Store the source branch when creating issue branches
- Use stored source branch for merge operations

### 3. Update Documentation and Descriptions

**Tool descriptions to update:**
- Issue merge tool descriptions
- CLI help text about merging
- Built-in prompt descriptions
- MCP tool documentation

**Remove references to "main branch" and replace with "base branch" or "source branch"**

## Implementation Steps

1. **Update Git Operations**
   - Modify `git.rs` to track source branches instead of assuming main
   - Update issue creation to allow any non-issue base branch
   - Update merge operations to merge back to stored source branch

2. **Update Issue Storage**
   - Store the source branch information when creating issues
   - Modify issue metadata to include base branch information

3. **Update Tool Descriptions**
   - Update MCP tool descriptions to reflect flexible branching
   - Update CLI help text
   - Update built-in prompt descriptions

4. **Update Tests**
   - Modify tests to work with flexible base branches
   - Add tests for feature branch → issue branch workflows
   - Test merge back to various base branches

5. **Update Documentation**
   - Update user documentation to explain flexible branching
   - Update examples to show feature branch workflows

## Benefits

- **Flexible Development Workflows**: Support feature branches, release branches, etc.
- **Team Collaboration**: Multiple developers can work on features with their own issue branches
- **Git Flow Compatibility**: Works with various Git workflow patterns
- **Backward Compatibility**: Still works with main/master branch workflows

## Edge Cases to Handle

- What happens if the source branch is deleted before merge? Abort
- How to handle merge conflicts between issue branch and changed source branch? Abort
- Should we allow changing the target branch for merge? No

## Validation

- Ensure issue branches can only be created from non-issue branches
- Verify merge operations target the correct source branch
- Test with various Git workflow patterns (Git Flow, GitHub Flow, etc.)