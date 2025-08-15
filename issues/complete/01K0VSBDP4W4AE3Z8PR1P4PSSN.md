failures:
    mcp::tests::mcp_integration_tests::test_mcp_git_branch_management

is failing on github actions

## Proposed Solution

After investigating the failing test, I found that the issue is related to git repository initialization in the test environment. The `create_test_mcp_server` function in the MCP tests does not explicitly set the default branch when initializing a git repository, which can cause inconsistent behavior between local environments and GitHub Actions CI.

### Root Cause Analysis

1. **Test runs locally**: The test passes on local development machines
2. **Test fails in CI**: GitHub Actions has different git default configurations
3. **Branch name assumptions**: The test assumes either "main" or "master" as the default branch but doesn't control which one is used during repository initialization
4. **Git version differences**: Different git versions and configurations may create different default branch names

### Implementation Plan

1. **Fix git repository initialization**: Modify `create_test_mcp_server` to explicitly set the default branch name to "main" during repository initialization
2. **Ensure consistent git configuration**: Add explicit branch creation and checkout commands to guarantee the test environment is predictable
3. **Improve error handling**: Add better error messages to help diagnose future issues
4. **Verify the fix**: Run tests locally and ensure they still pass while being more robust for CI environments

The fix will involve:
- Adding `git branch -M main` after `git init` to explicitly set the main branch
- Ensuring all git operations use consistent branch references
- Adding defensive checks for git operations in the test helper functions
