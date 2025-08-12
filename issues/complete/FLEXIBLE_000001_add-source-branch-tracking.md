# Add Source Branch Tracking to Issue Model

Refer to ./specification/flexible_base_branch_support.md

## Goal

Add infrastructure to track the source branch when creating issue branches, enabling flexible base branch support.

## Tasks

1. **Update Issue Model Structure**
   - Add `source_branch: String` field to Issue struct
   - Ensure backwards compatibility with existing issues

2. **Update Issue Storage Layer**
   - Modify issue creation to accept and store source branch
   - Update issue serialization/deserialization to handle new field
   - Default source branch to "main" for existing issues without source branch

3. **Update Issue Creation Logic**
   - Capture current branch as source branch when creating new issues
   - Validate that source branch is not an issue branch
   - Store source branch in issue metadata

## Implementation Details

- Location: `swissarmyhammer/src/issues/` and related issue storage code
- Add proper error handling for source branch validation
- Maintain backwards compatibility - existing issues should work unchanged
- Use consistent naming: `source_branch` throughout the codebase

## Testing Requirements

- Test issue creation with various source branches
- Test backwards compatibility with existing issues
- Test that issue storage correctly persists source branch
- Test validation prevents creating issues from issue branches

## Success Criteria

- Issue model contains source branch information  
- Issue creation captures and stores current branch as source
- Existing issues continue to work with default source branch
- All existing tests pass

This is the foundation for flexible base branch support - subsequent steps will use this source branch tracking.

## Proposed Solution

Based on the specification, I'll implement source branch tracking in the Issue model using the following approach:

### 1. Issue Model Enhancement
- Add `source_branch: String` field to the Issue struct 
- Ensure backward compatibility by providing a default value of "main" for existing issues
- Update Serde serialization to handle optional source branch field gracefully

### 2. Storage Layer Updates  
- Modify issue creation to accept and store the current branch as source branch
- Update deserialization logic to handle existing issues without source branch field
- Ensure consistent storage format across file-based issue storage

### 3. Issue Creation Logic
- Capture `git branch --show-current` output when creating new issues
- Validate that the current branch is not an issue branch (doesn't start with "issue/")
- Store the validated source branch in the issue metadata

### 4. Implementation Steps
1. Locate and examine the current Issue struct definition
2. Add the `source_branch` field with proper Serde attributes for backward compatibility
3. Update issue creation logic to capture and validate source branch
4. Test backward compatibility with existing issues
5. Verify that new issues properly store source branch information

This foundation will enable the subsequent flexible base branch support features outlined in the specification.