# PLAN_000008: Integration Tests for Workflow Execution

**Refer to ./specification/plan.md**

## Goal

Create comprehensive integration tests that verify the complete end-to-end functionality of the plan command, from CLI parsing through workflow execution to issue file creation.

## Background

While unit tests verify individual components, integration tests ensure the complete system works together correctly. We need tests that verify the plan command actually executes workflows, processes files, and creates the expected output.

## Requirements

1. Test complete workflow execution with real plan files
2. Verify issue file creation in the correct format
3. Test parameter passing through the entire chain
4. Test with various file formats and sizes
5. Verify error handling in realistic scenarios
6. Test interaction with file system and git repository state
7. Follow existing integration test patterns in the codebase

## Implementation Details

### Test Structure

Create integration tests in the appropriate location (likely `tests/` directory or in CLI integration tests):

```rust
#[tokio::test]
async fn test_plan_command_end_to_end() {
    // Create temporary directory for test
    let temp_dir = tempfile::tempdir().unwrap();
    let plan_file = temp_dir.path().join("test-plan.md");
    
    // Create a simple test plan file
    std::fs::write(&plan_file, r#"
# Test Plan

## Overview
This is a test specification for integration testing.

## Requirements
1. Create a simple component
2. Add basic functionality  
3. Write tests

## Implementation
Basic implementation steps.
    "#).unwrap();
    
    // Change to temp directory
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&temp_dir).unwrap();
    
    // Create issues directory
    std::fs::create_dir_all("issues").unwrap();
    
    // Execute plan command
    let result = execute_plan_command(plan_file.to_str().unwrap()).await;
    
    // Verify success
    assert!(result.is_ok(), "Plan command should succeed");
    
    // Check that issue files were created
    let issues_dir = temp_dir.path().join("issues");
    assert!(issues_dir.exists(), "Issues directory should exist");
    
    // Verify issue files are created with correct naming
    let issue_files = std::fs::read_dir(&issues_dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    
    assert!(!issue_files.is_empty(), "Should create at least one issue file");
    assert!(issue_files.iter().any(|f| f.contains("TEST")), "Should create files with test plan prefix");
    
    // Restore original directory
    std::env::set_current_dir(&original_dir).unwrap();
}

#[tokio::test]
async fn test_plan_command_with_existing_issues() {
    let temp_dir = tempfile::tempdir().unwrap();
    let plan_file = temp_dir.path().join("feature-plan.md");
    
    // Create test plan
    std::fs::write(&plan_file, r#"
# Feature Plan

## Goal
Add new feature to application.

## Steps
1. Design API
2. Implement backend
3. Create frontend
4. Add tests
    "#).unwrap();
    
    // Setup test environment
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&temp_dir).unwrap();
    
    // Create issues directory with existing issues
    std::fs::create_dir_all("issues").unwrap();
    std::fs::write("issues/EXISTING_000001_old-feature.md", "# Old Feature").unwrap();
    
    // Execute plan command
    let result = execute_plan_command(plan_file.to_str().unwrap()).await;
    
    assert!(result.is_ok());
    
    // Verify new issues don't conflict with existing ones
    let issue_files = std::fs::read_dir(temp_dir.path().join("issues"))
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    
    assert!(issue_files.iter().any(|f| f.starts_with("EXISTING_")));
    assert!(issue_files.iter().any(|f| f.starts_with("FEATURE")));
    
    std::env::set_current_dir(&original_dir).unwrap();
}

#[tokio::test]
async fn test_plan_command_file_not_found() {
    let result = execute_plan_command("nonexistent-plan.md").await;
    
    assert!(result.is_err(), "Should fail with file not found");
    
    let error_msg = format!("{:?}", result.unwrap_err());
    assert!(error_msg.contains("not found") || error_msg.contains("does not exist"));
}

#[tokio::test]
async fn test_plan_command_relative_path() {
    let temp_dir = tempfile::tempdir().unwrap();
    let plans_dir = temp_dir.path().join("plans");
    std::fs::create_dir_all(&plans_dir).unwrap();
    
    let plan_file = plans_dir.join("relative-test.md");
    std::fs::write(&plan_file, r#"
# Relative Path Test

Test planning with relative path.
    "#).unwrap();
    
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&temp_dir).unwrap();
    std::fs::create_dir_all("issues").unwrap();
    
    // Test with relative path
    let result = execute_plan_command("./plans/relative-test.md").await;
    
    assert!(result.is_ok(), "Should handle relative paths correctly");
    
    std::env::set_current_dir(&original_dir).unwrap();
}

#[tokio::test]
async fn test_plan_command_absolute_path() {
    let temp_dir = tempfile::tempdir().unwrap();
    let plan_file = temp_dir.path().join("absolute-test.md");
    
    std::fs::write(&plan_file, r#"
# Absolute Path Test

Test planning with absolute path.
    "#).unwrap();
    
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&temp_dir).unwrap();
    std::fs::create_dir_all("issues").unwrap();
    
    // Test with absolute path
    let result = execute_plan_command(plan_file.to_str().unwrap()).await;
    
    assert!(result.is_ok(), "Should handle absolute paths correctly");
    
    std::env::set_current_dir(&original_dir).unwrap();
}

// Helper function to execute plan command
async fn execute_plan_command(plan_filename: &str) -> Result<(), Box<dyn std::error::Error>> {
    // This would call the actual implementation
    // Following the pattern from existing integration tests
    use swissarmyhammer_cli::execute_workflow; // Assuming this is the function
    
    let vars = vec![
        ("plan_filename".to_string(), plan_filename.to_string())
    ];
    
    execute_workflow("plan", vars, Vec::new(), false, false, false, None, false).await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}
```

### Test Categories

### 1. End-to-End Success Tests
- Complete workflow execution with valid plan files
- Issue file creation verification
- Parameter passing validation
- Output format verification

### 2. File System Tests
- Relative path handling
- Absolute path handling
- Files with spaces in names
- Unicode filenames
- Large plan files

### 3. Error Handling Tests
- File not found scenarios
- Permission denied scenarios
- Invalid file formats
- Directory instead of file
- Empty files

### 4. State Management Tests
- Interaction with existing issues
- Git repository state handling  
- Memo system integration
- Configuration file interaction

### 5. Performance Tests
- Large plan file processing
- Multiple concurrent executions
- Memory usage validation
- Timeout handling

## Test Environment Setup

```rust
// Common test setup helper
fn setup_test_environment() -> (tempfile::TempDir, std::path::PathBuf) {
    let temp_dir = tempfile::tempdir().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    
    std::env::set_current_dir(&temp_dir).unwrap();
    std::fs::create_dir_all("issues").unwrap();
    std::fs::create_dir_all(".swissarmyhammer/tmp").unwrap();
    
    (temp_dir, original_dir)
}

fn teardown_test_environment(original_dir: std::path::PathBuf) {
    std::env::set_current_dir(&original_dir).unwrap();
}
```

## Implementation Steps

1. Research existing integration test patterns in the codebase
2. Identify the correct location for integration tests
3. Create helper functions for test setup and teardown
4. Implement basic end-to-end success test
5. Add file system and path handling tests
6. Implement error scenario tests
7. Add state management and interaction tests
8. Create performance and edge case tests
9. Ensure all tests are deterministic and isolated
10. Add documentation for running integration tests

## Acceptance Criteria

- [ ] End-to-end workflow execution tests pass
- [ ] Issue file creation is verified correctly
- [ ] File path handling tests (relative/absolute) work
- [ ] Error scenarios are properly tested
- [ ] Tests are isolated and don't interfere with each other
- [ ] Performance tests validate reasonable execution times
- [ ] All tests are deterministic and repeatable
- [ ] Test documentation is complete

## Testing Commands

```bash
# Run integration tests
cargo test --test integration

# Run specific plan integration tests  
cargo test test_plan_command

# Run with output
cargo test test_plan_command -- --nocapture
```

## Dependencies

- Requires all previous steps (PLAN_000001 through PLAN_000007)
- Needs access to workflow execution system
- Requires temporary file system setup
- May need mock or test utilities

## Notes

- Use `tempfile` crate for isolated test environments
- Ensure tests don't interfere with real project state
- Test both success and failure scenarios thoroughly
- Use realistic plan file content for testing
- Consider testing with different file encodings
- Verify cleanup happens properly after tests
- Test execution should be fast enough for CI/CD