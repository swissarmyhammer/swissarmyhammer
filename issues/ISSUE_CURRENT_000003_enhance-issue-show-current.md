# Enhance issue_show Tool with "current" Parameter

Refer to ./specification/issue_current.md

## Goal

Add support for `"current"` as a special parameter value in the `issue_show` tool to replace `issue_current` functionality.

## Tasks

1. **Update ShowIssueRequest struct**:
   - Ensure `name` parameter can handle special values
   - Add validation for special parameter values
   - Maintain backward compatibility with regular issue names

2. **Implement current issue detection logic**:
   - Add method to detect when `name == "current"`
   - Integrate git branch parsing logic from `CurrentIssueTool`
   - Use same config for `issue_branch_prefix` as current tool
   - Handle case when not on an issue branch (return appropriate message)

3. **Add git operations integration**:
   - Access `context.git_ops` in the execute method
   - Handle git operations not available case
   - Parse branch name to extract issue name using same logic as `CurrentIssueTool`
   - Maintain same error handling patterns

4. **Preserve existing behavior**:
   - Ensure regular issue names continue to work exactly as before
   - Maintain same response formatting for backward compatibility
   - Keep same error handling for invalid issue names

5. **Handle edge cases**:
   - Not on an issue branch: return descriptive message
   - Git operations unavailable: return appropriate error
   - Invalid branch name format: handle gracefully

## Expected Outcome

`issue_show current` works identically to `issue_current` tool:
- Returns current issue details when on an issue branch
- Returns informative message when not on an issue branch
- Handles all error cases appropriately
- Maintains backward compatibility for regular issue names

## Success Criteria

- `issue_show current` returns identical results to `issue_current`
- All existing `issue_show` functionality remains unchanged
- Edge cases are handled consistently
- Code follows established patterns and error handling