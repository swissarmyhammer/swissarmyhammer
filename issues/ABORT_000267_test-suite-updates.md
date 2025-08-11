# Update Test Suite for New Abort System

Refer to ./specification/abort.md

## Objective
Update all existing test files that relied on string-based "ABORT ERROR" detection to use the new file-based abort system, ensuring comprehensive test coverage is maintained.

## Context
Multiple test files throughout the codebase test abort functionality using the old string-based approach. These tests need to be updated to work with the new file-based system while maintaining the same level of coverage and validation.

## Tasks

### 1. Update CLI MCP Integration Tests
Location: `swissarmyhammer-cli/tests/cli_mcp_integration_test.rs:278-282`
- Remove string-based abort error testing
- Add tests for new ExecutorError::Abort handling
- Test CLI response to file-based abort
- Maintain test coverage for MCP integration scenarios

### 2. Update Dedicated Abort Error CLI Tests
Location: `swissarmyhammer-cli/tests/abort_error_cli_test.rs`
- Complete rewrite to test file-based abort system
- Test abort tool → file creation → CLI detection flow
- Test proper exit codes with new system
- Test error message formatting with abort reasons

### 3. Update Abort Error Pattern Tests
Location: `swissarmyhammer/tests/abort_error_pattern_tests.rs`
- Replace string pattern tests with file-based detection tests
- Test abort file creation and detection patterns
- Test atomic file operations in test scenarios

### 4. Update Abort Error Integration Tests
Location: `swissarmyhammer/tests/abort_error_integration_tests.rs`
- Update integration tests to use file-based abort
- Test workflow executor abort detection
- Test end-to-end abort scenarios with new system

### 5. Update Workflow Action Tests
Location: `swissarmyhammer/src/workflow/actions_tests/prompt_action_tests.rs`
- Update prompt action tests that rely on abort functionality
- Ensure prompt actions work with new abort system
- Test abort handling in action execution

### 6. Clean Up Test Utilities
- Update test helper functions for abort testing
- Create new utilities for file-based abort testing
- Remove obsolete string-based test utilities
- Add utilities for abort file manipulation in tests

## Implementation Details

### Test Update Strategy
```rust
// Replace string-based test patterns like:
#[test]
fn test_abort_error_detection() {
    let output = run_command(&["test", "abort_prompt"]);
    assert!(output.contains("ABORT ERROR"));
}

// With file-based test patterns like:
#[test]
fn test_abort_file_detection() {
    let temp_dir = create_test_environment();
    run_abort_tool_command("Test abort reason");
    
    assert!(temp_dir.path().join(".swissarmyhammer/.abort").exists());
    let content = std::fs::read_to_string(temp_dir.path().join(".swissarmyhammer/.abort")).unwrap();
    assert_eq!(content, "Test abort reason");
}
```

### Test Helper Functions
```rust
// Create new test utilities
fn create_abort_file(reason: &str) -> Result<()> {
    std::fs::create_dir_all(".swissarmyhammer")?;
    std::fs::write(".swissarmyhammer/.abort", reason)?;
    Ok(())
}

fn cleanup_abort_file() -> Result<()> {
    let _ = std::fs::remove_file(".swissarmyhammer/.abort");
    Ok(())
}

fn assert_abort_file_contains(expected_reason: &str) {
    let content = std::fs::read_to_string(".swissarmyhammer/.abort")
        .expect("Abort file should exist");
    assert_eq!(content, expected_reason);
}
```

### Integration Test Updates
- Test complete abort flow from MCP tool to CLI exit
- Test abort file cleanup between test runs
- Test concurrent abort scenarios
- Test abort with various workflow types

## Validation Criteria
- [ ] All abort-related tests are updated for file-based system
- [ ] No tests rely on string-based "ABORT ERROR" detection
- [ ] Test coverage is maintained or improved
- [ ] New test utilities support file-based abort testing
- [ ] All tests pass with new abort system
- [ ] Test isolation is maintained (no cross-test contamination)
- [ ] Performance of test suite is maintained

## Testing Requirements
- Update all existing abort tests to use new system
- Add new tests for file-based abort functionality
- Ensure test isolation with proper cleanup
- Maintain comprehensive coverage of abort scenarios

## Files to Modify
Based on specification analysis:
- `swissarmyhammer-cli/tests/abort_error_cli_test.rs`
- `swissarmyhammer-cli/tests/cli_mcp_integration_test.rs`
- `swissarmyhammer/tests/abort_error_pattern_tests.rs`
- `swissarmyhammer/tests/abort_error_integration_tests.rs`
- `swissarmyhammer/src/workflow/actions_tests/prompt_action_tests.rs`

## Dependencies
- ABORT_000266_string-detection-removal (old system must be removed)
- ABORT_000265_comprehensive-testing (new tests must be in place)

## Follow-up Issues
- ABORT_000268_documentation-updates