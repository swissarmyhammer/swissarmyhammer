You should never branch an issue from an issue branch, if we end up in that spot, we need to call the abort tool and abort.

If we are already on the correct branch for the issue, then there is no error -- and no need to branch.
You should never branch an issue from an issue branch, if we end up in that spot, we need to call the abort tool and abort.

If we are already on the correct branch for the issue, then there is no error -- and no need to branch.

## Proposed Solution

The issue is in the validation logic in `GitOperations::create_work_branch_with_source` and the MCP `issue_work` tool. The problem occurs when:

1. We're on an issue branch (e.g., `issue/branch-A`)  
2. We try to work on a different issue (e.g., `issue/branch-B`)
3. The validation should detect this invalid scenario and call the abort tool

**Current Logic Analysis:**

In `swissarmyhammer-tools/src/mcp/tools/issues/work/mod.rs`, lines 71-79:
- The validation checks if we're on an issue branch AND trying to work on a different issue
- But this only returns an MCP error, it doesn't call the abort tool as required

**The Fix:**

1. **Enhanced Validation**: When we detect branching from an issue branch to a different issue branch, we need to call the `abort_create` tool instead of just returning an error

2. **Precise Condition**: The abort should trigger when:
   - Current branch starts with `issue/` 
   - Target branch is different from current branch
   - Target branch also starts with `issue/`
   - This represents an invalid issue-to-issue branching attempt

3. **Implementation Steps**:
   - Modify the issue_work tool to call `mcp__sah__abort_create` when this condition is detected
   - Provide a clear abort reason explaining the circular dependency violation
   - Ensure the abort is properly propagated through the system

This will prevent the circular dependency issue by stopping the workflow when someone tries to branch an issue from another issue branch.