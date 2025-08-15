# Update Tests to Use Enhanced issue_show

Refer to ./specification/issue_current.md

## Goal

Update all test files that reference the deprecated `issue_current` and `issue_next` tools to use the new `issue_show current` and `issue_show next` functionality.

## Tasks

1. **Identify affected test files**:
   - Search test directories for references to `issue_current` and `issue_next`
   - Document all test files that need updating
   - Analyze test scenarios to understand required changes

2. **Update CLI integration tests**:
   - Update `/swissarmyhammer-cli/tests/comprehensive_cli_mcp_integration_tests.rs`
   - Update `/swissarmyhammer-cli/tests/cli_mcp_integration_test.rs`
   - Change tool calls from old tools to new `issue_show` syntax
   - Verify test assertions still validate correct behavior

3. **Update MCP tool tests**:
   - Remove tests for `CurrentIssueTool` and `NextIssueTool` if they exist
   - Update any integration tests that use these tools directly
   - Ensure new `issue_show` functionality is properly tested

4. **Update CLI command tests**:
   - Update `/swissarmyhammer-cli/src/issue.rs` tests if they exist
   - Change any test commands that use old tool syntax
   - Verify CLI integration with new tool functionality

5. **Fix broken test references**:
   - Fix any tests that might be calling the old tools directly
   - Update test assertions to match new response formats
   - Ensure test scenarios cover both success and error cases

6. **Validate test coverage**:
   - Ensure comprehensive testing of new special parameter functionality
   - Verify error cases are properly tested
   - Run all tests to ensure they pass consistently

## Expected Outcome

All tests updated to use new consolidated tool syntax:
- Same test coverage and validation
- Updated tool references throughout test suite
- All tests pass consistently
- No references to deprecated tools in tests

## Success Criteria

- All test files are updated to use new tool syntax
- All tests pass without errors
- Test coverage remains comprehensive
- No references to old tools remain in test code
- CI/CD pipeline passes all test stages