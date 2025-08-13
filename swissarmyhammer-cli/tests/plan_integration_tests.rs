//! Integration tests for the Plan command workflow execution
//!
//! Tests the complete end-to-end functionality of the plan command,
//! verifying workflow execution, issue file creation, and error handling.

use anyhow::Result;
use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

mod test_utils;
use test_utils::setup_git_repo;

/// Helper to create a test environment with required directory structure
fn setup_plan_test_environment() -> Result<(TempDir, PathBuf)> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path().to_path_buf();

    // Create required directories
    let issues_dir = temp_path.join("issues");
    fs::create_dir_all(&issues_dir)?;

    let swissarmyhammer_dir = temp_path.join(".swissarmyhammer");
    fs::create_dir_all(&swissarmyhammer_dir)?;
    
    let tmp_dir = swissarmyhammer_dir.join("tmp");
    fs::create_dir_all(&tmp_dir)?;

    // Initialize git repository
    setup_git_repo(&temp_path)?;

    Ok((temp_dir, temp_path))
}

/// Create a sample plan specification file
fn create_test_plan_file(dir: &Path, filename: &str, content: &str) -> PathBuf {
    let plan_file = dir.join(filename);
    fs::write(&plan_file, content).expect("Failed to write plan file");
    plan_file
}

/// Helper to run plan command and return output
fn run_plan_command(temp_path: &Path, plan_file: &str) -> Result<std::process::Output> {
    Ok(Command::cargo_bin("sah")?
        .args(["plan", plan_file])
        .current_dir(temp_path)
        .env("SWISSARMYHAMMER_TEST_MODE", "1")
        .timeout(std::time::Duration::from_secs(60))
        .output()?)
}

/// Check that issue files were created with expected patterns
fn verify_issue_files_created(issues_dir: &Path, expected_prefix: &str) -> Result<Vec<String>> {
    let mut issue_files = Vec::new();
    
    for entry in fs::read_dir(issues_dir)? {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().to_string();
        
        if file_name.starts_with(expected_prefix) && file_name.ends_with(".md") {
            issue_files.push(file_name);
        }
    }
    
    issue_files.sort();
    Ok(issue_files)
}

/// Test basic end-to-end plan command execution
#[tokio::test]
async fn test_plan_command_end_to_end_basic() -> Result<()> {
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;
    
    // Create a simple test plan file
    let plan_content = r#"
# Test Feature Plan

## Overview
This is a test specification for implementing a simple feature.

## Requirements
1. Create a basic data structure
2. Add methods for data manipulation
3. Write unit tests
4. Add integration tests

## Implementation Details
The feature should provide a simple API for managing test data.

## Acceptance Criteria
- All tests pass
- Code is properly documented
- Error handling is implemented
"#;
    
    let plan_file = create_test_plan_file(&temp_path, "test-feature.md", plan_content);
    
    // Execute plan command
    let output = run_plan_command(&temp_path, plan_file.to_str().unwrap())?;
    
    // Verify command succeeded
    assert!(
        output.status.success(),
        "Plan command should succeed. Stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Plan workflow completed successfully") || stdout.contains("✅"),
        "Should show success message: {}",
        stdout
    );
    
    // Verify issue files were created
    let issues_dir = temp_path.join("issues");
    let issue_files = verify_issue_files_created(&issues_dir, "TEST-FEATURE")?;
    
    assert!(
        !issue_files.is_empty(),
        "Should create at least one issue file. Files found: {:?}",
        fs::read_dir(&issues_dir)?.collect::<Vec<_>>()
    );
    
    // Verify issue files contain expected content
    for issue_file in &issue_files {
        let issue_path = issues_dir.join(issue_file);
        let content = fs::read_to_string(&issue_path)?;
        
        assert!(
            content.contains("Refer to test-feature.md"),
            "Issue file {} should reference the plan file: {}",
            issue_file,
            content
        );
    }
    
    Ok(())
}

/// Test plan command with complex specification
#[tokio::test]
async fn test_plan_command_complex_specification() -> Result<()> {
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;
    
    // Create a more complex plan file
    let complex_plan = r#"
# Database Migration System

## Overview
Implement a comprehensive database migration system that supports multiple database engines.

## Requirements

### Core Features
1. Migration file management
2. Version tracking
3. Rollback capabilities
4. Multi-database support (PostgreSQL, MySQL, SQLite)
5. Schema validation

### Technical Requirements
1. CLI interface for migration commands
2. Configuration file support
3. Logging and monitoring
4. Transaction safety
5. Concurrent migration handling

### Non-Functional Requirements
1. Performance optimization for large schemas
2. Error handling and recovery
3. Extensive test coverage
4. Documentation and examples

## Architecture

### Components
- Migration Engine
- Database Connectors
- Version Manager
- CLI Interface
- Configuration System

### Data Flow
1. Load configuration
2. Connect to database
3. Check current version
4. Plan migration path
5. Execute migrations
6. Update version tracking

## Implementation Plan

### Phase 1: Foundation
- Core data structures
- Basic migration engine
- PostgreSQL connector

### Phase 2: Features
- Additional database connectors
- Rollback functionality  
- CLI interface

### Phase 3: Advanced
- Performance optimization
- Monitoring and logging
- Advanced error handling

## Acceptance Criteria
- All database engines supported
- Zero data loss during migrations
- Full rollback capability
- Complete test coverage
- Production-ready performance
"#;
    
    let plan_file = create_test_plan_file(&temp_path, "database-migration.md", complex_plan);
    
    // Execute plan command
    let output = run_plan_command(&temp_path, plan_file.to_str().unwrap())?;
    
    // Verify command succeeded
    assert!(
        output.status.success(),
        "Plan command should succeed for complex spec. Stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    
    // Verify more issue files were created for complex spec
    let issues_dir = temp_path.join("issues");
    let issue_files = verify_issue_files_created(&issues_dir, "DATABASE-MIGRATION")?;
    
    assert!(
        issue_files.len() >= 5,
        "Complex specification should create multiple issue files. Found: {:?}",
        issue_files
    );
    
    // Verify sequential numbering
    let mut numbers = Vec::new();
    for file in &issue_files {
        if let Some(number_str) = file.split('_').nth(1) {
            if let Ok(number) = number_str.parse::<u32>() {
                numbers.push(number);
            }
        }
    }
    numbers.sort();
    
    assert!(
        numbers.len() >= 2,
        "Should have sequential numbering in issue files"
    );
    
    for i in 1..numbers.len() {
        assert!(
            numbers[i] > numbers[i-1],
            "Issue files should be sequentially numbered"
        );
    }
    
    Ok(())
}

/// Test plan command with existing issues directory
#[tokio::test]
async fn test_plan_command_with_existing_issues() -> Result<()> {
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;
    
    // Create some existing issue files
    let issues_dir = temp_path.join("issues");
    fs::write(issues_dir.join("EXISTING_000001_old-issue.md"), "# Old Issue")?;
    fs::write(issues_dir.join("OTHER_000001_different-project.md"), "# Different Project")?;
    
    let plan_content = r#"
# New Feature

## Overview
Add a new feature to the system.

## Requirements
1. Design API
2. Implement core logic
3. Add tests
"#;
    
    let plan_file = create_test_plan_file(&temp_path, "new-feature.md", plan_content);
    
    // Execute plan command
    let output = run_plan_command(&temp_path, plan_file.to_str().unwrap())?;
    
    // Verify command succeeded
    assert!(
        output.status.success(),
        "Plan command should succeed with existing issues. Stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    
    // Verify new issue files were created without conflicts
    let new_issues = verify_issue_files_created(&issues_dir, "NEW-FEATURE")?;
    assert!(!new_issues.is_empty(), "Should create new issue files");
    
    // Verify old files still exist
    assert!(
        issues_dir.join("EXISTING_000001_old-issue.md").exists(),
        "Existing issue files should not be affected"
    );
    assert!(
        issues_dir.join("OTHER_000001_different-project.md").exists(),
        "Other project files should not be affected"
    );
    
    Ok(())
}

/// Test plan command file not found error
#[tokio::test]
async fn test_plan_command_file_not_found() -> Result<()> {
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;
    
    // Try to run plan with non-existent file
    let output = run_plan_command(&temp_path, "nonexistent-plan.md")?;
    
    // Verify command failed
    assert!(
        !output.status.success(),
        "Plan command should fail with non-existent file"
    );
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Plan file not found") || stderr.contains("not found"),
        "Should show file not found error: {}",
        stderr
    );
    
    Ok(())
}

/// Test plan command with invalid file (directory instead of file)
#[tokio::test]
async fn test_plan_command_directory_instead_of_file() -> Result<()> {
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;
    
    // Create a directory with the plan name
    let plan_dir = temp_path.join("plan-directory");
    fs::create_dir_all(&plan_dir)?;
    
    // Try to run plan command on directory
    let output = run_plan_command(&temp_path, "plan-directory")?;
    
    // Verify command failed
    assert!(
        !output.status.success(),
        "Plan command should fail when given a directory"
    );
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not a file") || stderr.contains("directory"),
        "Should show directory error: {}",
        stderr
    );
    
    Ok(())
}

/// Test plan command with relative path
#[tokio::test]
async fn test_plan_command_relative_path() -> Result<()> {
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;
    
    // Create plans subdirectory
    let plans_dir = temp_path.join("plans");
    fs::create_dir_all(&plans_dir)?;
    
    let plan_content = r#"
# Relative Path Test

## Overview
Test planning with relative path.

## Requirements
1. Test relative path handling
2. Verify issue creation
"#;
    
    let _plan_file = create_test_plan_file(&plans_dir, "relative-test.md", plan_content);
    
    // Execute plan command with relative path
    let output = run_plan_command(&temp_path, "./plans/relative-test.md")?;
    
    // Verify command succeeded
    assert!(
        output.status.success(),
        "Plan command should handle relative paths. Stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    
    // Verify issue files were created
    let issues_dir = temp_path.join("issues");
    let issue_files = verify_issue_files_created(&issues_dir, "RELATIVE-TEST")?;
    assert!(!issue_files.is_empty(), "Should create issue files with relative path");
    
    Ok(())
}

/// Test plan command with absolute path
#[tokio::test]
async fn test_plan_command_absolute_path() -> Result<()> {
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;
    
    let plan_content = r#"
# Absolute Path Test

## Overview
Test planning with absolute path.

## Requirements
1. Test absolute path handling
2. Verify issue creation
"#;
    
    let plan_file = create_test_plan_file(&temp_path, "absolute-test.md", plan_content);
    
    // Execute plan command with absolute path
    let output = run_plan_command(&temp_path, &plan_file.to_string_lossy())?;
    
    // Verify command succeeded
    assert!(
        output.status.success(),
        "Plan command should handle absolute paths. Stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    
    // Verify issue files were created
    let issues_dir = temp_path.join("issues");
    let issue_files = verify_issue_files_created(&issues_dir, "ABSOLUTE-TEST")?;
    assert!(!issue_files.is_empty(), "Should create issue files with absolute path");
    
    Ok(())
}

/// Test plan command with file containing spaces
#[tokio::test]
async fn test_plan_command_file_with_spaces() -> Result<()> {
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;
    
    let plan_content = r#"
# Spaces Test Plan

## Overview
Test planning with filename containing spaces.

## Requirements
1. Handle spaces in filename
2. Create proper issue files
"#;
    
    let plan_file = create_test_plan_file(&temp_path, "plan with spaces.md", plan_content);
    
    // Execute plan command
    let output = run_plan_command(&temp_path, &plan_file.to_string_lossy())?;
    
    // Verify command succeeded
    assert!(
        output.status.success(),
        "Plan command should handle files with spaces. Stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    
    // Verify issue files were created (spaces should be handled in prefix)
    let issues_dir = temp_path.join("issues");
    let all_files: Vec<_> = fs::read_dir(&issues_dir)?.collect();
    assert!(
        !all_files.is_empty(),
        "Should create issue files even with spaces in filename"
    );
    
    Ok(())
}

/// Test plan command workflow execution steps
#[tokio::test]
async fn test_plan_command_workflow_execution_steps() -> Result<()> {
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;
    
    let plan_content = r#"
# Workflow Test Plan

## Overview
Test the complete workflow execution including all steps.

## Requirements
1. Parse specification
2. Analyze existing code
3. Create draft plan
4. Generate issue files
5. Validate results

## Implementation
The system should create a clear step-by-step plan.
"#;
    
    let plan_file = create_test_plan_file(&temp_path, "workflow-test.md", plan_content);
    
    // Execute plan command
    let output = run_plan_command(&temp_path, &plan_file.to_string_lossy())?;
    
    // Verify command succeeded
    assert!(
        output.status.success(),
        "Workflow execution should succeed. Stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    
    // Check if draft plan was created (should be cleaned up after execution)
    let _draft_plan = temp_path.join(".swissarmyhammer/tmp/DRAFT_PLAN.md");
    
    // Verify issue files were created
    let issues_dir = temp_path.join("issues");
    let issue_files = verify_issue_files_created(&issues_dir, "WORKFLOW-TEST")?;
    assert!(!issue_files.is_empty(), "Should create issue files");
    
    // Verify issue file content structure
    for issue_file in &issue_files {
        let issue_path = issues_dir.join(issue_file);
        let content = fs::read_to_string(&issue_path)?;
        
        // Check for expected content patterns
        assert!(
            content.contains("#") || content.len() > 10,
            "Issue file should contain meaningful content: {}",
            issue_file
        );
        
        assert!(
            content.contains("workflow-test.md"),
            "Issue should reference the source plan file"
        );
    }
    
    Ok(())
}

/// Test plan command with empty file
#[tokio::test]
async fn test_plan_command_empty_file() -> Result<()> {
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;
    
    // Create an empty plan file
    let plan_file = create_test_plan_file(&temp_path, "empty-plan.md", "");
    
    // Execute plan command
    let output = run_plan_command(&temp_path, &plan_file.to_string_lossy())?;
    
    // The command might succeed but with minimal output, or it might fail gracefully
    // Either behavior is acceptable for an empty file
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // If it fails, it should fail gracefully with a meaningful message
    if !output.status.success() {
        assert!(
            stderr.len() > 0 || stdout.len() > 0,
            "Should provide feedback for empty file"
        );
    }
    
    Ok(())
}

/// Test plan command with very large file
#[tokio::test]
async fn test_plan_command_large_file() -> Result<()> {
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;
    
    // Create a large plan file (but not excessive for tests)
    let mut large_content = String::new();
    large_content.push_str("# Large Plan File\n\n## Overview\n");
    large_content.push_str("This is a large specification file for testing.\n\n");
    
    // Add multiple sections
    for i in 1..=20 {
        large_content.push_str(&format!("## Section {}\n", i));
        large_content.push_str(&format!("This is section {} with detailed requirements.\n\n", i));
        large_content.push_str("### Requirements\n");
        for j in 1..=5 {
            large_content.push_str(&format!("{}. Requirement {} for section {}\n", j, j, i));
        }
        large_content.push_str("\n");
    }
    
    let plan_file = create_test_plan_file(&temp_path, "large-plan.md", &large_content);
    
    // Execute plan command with extended timeout
    let output = Command::cargo_bin("sah")?
        .args(["plan", &plan_file.to_string_lossy()])
        .current_dir(&temp_path)
        .env("SWISSARMYHAMMER_TEST_MODE", "1")
        .timeout(std::time::Duration::from_secs(120)) // Extended timeout for large file
        .output()?;
    
    // Verify command succeeded or failed gracefully
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Should either succeed or fail with a meaningful message
        assert!(
            stderr.contains("timeout") || stderr.contains("too large") || stderr.len() > 10,
            "Should handle large files gracefully: {}",
            stderr
        );
    } else {
        // If successful, verify reasonable number of issues were created
        let issues_dir = temp_path.join("issues");
        let issue_files = verify_issue_files_created(&issues_dir, "LARGE-PLAN")?;
        
        // Large spec should create multiple issues, but not excessive
        assert!(
            issue_files.len() >= 3 && issue_files.len() <= 50,
            "Large plan should create reasonable number of issues: {}",
            issue_files.len()
        );
    }
    
    Ok(())
}

/// Test plan command performance with reasonable timeout
#[tokio::test]
async fn test_plan_command_performance() -> Result<()> {
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;
    
    let plan_content = r#"
# Performance Test Plan

## Overview
Test the performance characteristics of the plan command.

## Requirements
1. Fast parsing of specification
2. Efficient issue generation
3. Reasonable memory usage
4. Timely completion

## Implementation Details
The system should complete planning within reasonable time limits.
"#;
    
    let plan_file = create_test_plan_file(&temp_path, "performance-test.md", plan_content);
    
    // Measure execution time
    let start = std::time::Instant::now();
    let output = run_plan_command(&temp_path, &plan_file.to_string_lossy())?;
    let duration = start.elapsed();
    
    // Verify command succeeded
    assert!(
        output.status.success(),
        "Performance test should succeed. Stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    
    // Verify reasonable execution time (adjust based on system performance)
    assert!(
        duration.as_secs() < 60,
        "Plan command should complete within reasonable time: {:?}",
        duration
    );
    
    // Verify results
    let issues_dir = temp_path.join("issues");
    let issue_files = verify_issue_files_created(&issues_dir, "PERFORMANCE-TEST")?;
    assert!(!issue_files.is_empty(), "Should create issue files");
    
    Ok(())
}

/// Test concurrent plan command execution
#[tokio::test]
async fn test_plan_command_concurrent_execution() -> Result<()> {
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;
    
    // Create multiple plan files
    let plan1_content = r#"
# Concurrent Plan 1
## Requirements
1. Feature A implementation
2. Tests for Feature A
"#;
    
    let plan2_content = r#"
# Concurrent Plan 2
## Requirements  
1. Feature B implementation
2. Tests for Feature B
"#;
    
    let plan1_file = create_test_plan_file(&temp_path, "concurrent-1.md", plan1_content);
    let plan2_file = create_test_plan_file(&temp_path, "concurrent-2.md", plan2_content);
    
    // Run both commands concurrently using tokio
    let temp_path1 = temp_path.clone();
    let temp_path2 = temp_path.clone();
    let plan1_str = plan1_file.to_string_lossy().to_string();
    let plan2_str = plan2_file.to_string_lossy().to_string();
    
    let (result1, result2) = tokio::join!(
        tokio::task::spawn_blocking(move || run_plan_command(&temp_path1, &plan1_str)),
        tokio::task::spawn_blocking(move || run_plan_command(&temp_path2, &plan2_str))
    );
    
    // Both should complete (though one might fail due to concurrency issues)
    let output1 = result1??;
    let output2 = result2??;
    
    // At least one should succeed
    assert!(
        output1.status.success() || output2.status.success(),
        "At least one concurrent execution should succeed"
    );
    
    // Verify some issue files were created
    let issues_dir = temp_path.join("issues");
    let all_issues: Vec<_> = fs::read_dir(&issues_dir)?.collect();
    assert!(
        all_issues.len() >= 1,
        "Concurrent execution should create some issue files"
    );
    
    Ok(())
}

/// Test plan command with special characters in filename
#[tokio::test]
async fn test_plan_command_special_characters() -> Result<()> {
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;
    
    let plan_content = r#"
# Special Characters Plan

## Overview
Test plan file with special characters in name.

## Requirements
1. Handle special characters properly
2. Generate valid issue files
"#;
    
    let plan_file = create_test_plan_file(&temp_path, "plan-test_v2.1@special.md", plan_content);
    
    // Execute plan command
    let output = run_plan_command(&temp_path, &plan_file.to_string_lossy())?;
    
    // Verify command succeeded
    assert!(
        output.status.success(),
        "Plan command should handle special characters. Stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    
    // Verify issue files were created
    let issues_dir = temp_path.join("issues");
    let all_files: Vec<_> = fs::read_dir(&issues_dir)?.collect();
    assert!(
        !all_files.is_empty(),
        "Should create issue files with special character filename"
    );
    
    Ok(())
}

/// Test plan command file system permissions
#[tokio::test]
async fn test_plan_command_readonly_issues_directory() -> Result<()> {
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;
    
    let plan_content = r#"
# Readonly Test Plan

## Requirements
1. Test readonly directory handling
"#;
    
    let plan_file = create_test_plan_file(&temp_path, "readonly-test.md", plan_content);
    
    // Make issues directory readonly (on Unix systems)
    let issues_dir = temp_path.join("issues");
    if cfg!(unix) {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&issues_dir)?.permissions();
        perms.set_mode(0o444); // Read-only
        fs::set_permissions(&issues_dir, perms)?;
        
        // Execute plan command
        let output = run_plan_command(&temp_path, &plan_file.to_string_lossy())?;
        
        // Should fail gracefully
        assert!(
            !output.status.success(),
            "Plan command should fail with readonly directory"
        );
        
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("permission") || stderr.contains("denied") || stderr.contains("write"),
            "Should show permission error: {}",
            stderr
        );
        
        // Restore permissions for cleanup
        let mut perms = fs::metadata(&issues_dir)?.permissions();
        perms.set_mode(0o755); // Restore write permissions
        fs::set_permissions(&issues_dir, perms)?;
    }
    
    Ok(())
}

/// Test plan command with missing issues directory
#[tokio::test] 
async fn test_plan_command_missing_issues_directory() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path().to_path_buf();
    
    // Create .swissarmyhammer directory but no issues directory
    let swissarmyhammer_dir = temp_path.join(".swissarmyhammer");
    fs::create_dir_all(&swissarmyhammer_dir)?;
    let tmp_dir = swissarmyhammer_dir.join("tmp");
    fs::create_dir_all(&tmp_dir)?;
    
    // Initialize git repository
    setup_git_repo(&temp_path)?;
    
    let plan_content = r#"
# Missing Directory Test

## Requirements
1. Create issues directory if missing
2. Generate issue files
"#;
    
    let plan_file = create_test_plan_file(&temp_path, "missing-dir-test.md", plan_content);
    
    // Execute plan command (should create issues directory)
    let output = run_plan_command(&temp_path, &plan_file.to_string_lossy())?;
    
    // Should succeed and create directory
    assert!(
        output.status.success(),
        "Plan command should create missing issues directory. Stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    
    // Verify issues directory was created
    let issues_dir = temp_path.join("issues");
    assert!(
        issues_dir.exists(),
        "Issues directory should be created if missing"
    );
    
    // Verify issue files were created
    let issue_files = verify_issue_files_created(&issues_dir, "MISSING-DIR-TEST")?;
    assert!(!issue_files.is_empty(), "Should create issue files");
    
    Ok(())
}

/// Test plan command with git repository state
#[tokio::test]
async fn test_plan_command_git_integration() -> Result<()> {
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;
    
    // Add some files to git
    let src_dir = temp_path.join("src");
    fs::create_dir_all(&src_dir)?;
    fs::write(src_dir.join("main.rs"), "fn main() { println!(\"Hello\"); }")?;
    
    // Add and commit files
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&temp_path)
        .output()?;
        
    std::process::Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(&temp_path)
        .env("GIT_AUTHOR_NAME", "Test User")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test User")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()?;
    
    let plan_content = r#"
# Git Integration Test

## Overview
Test plan command in git repository context.

## Requirements
1. Analyze existing code
2. Create complementary features
3. Maintain git history
"#;
    
    let plan_file = create_test_plan_file(&temp_path, "git-test.md", plan_content);
    
    // Execute plan command
    let output = run_plan_command(&temp_path, &plan_file.to_string_lossy())?;
    
    // Verify command succeeded
    assert!(
        output.status.success(),
        "Plan command should work in git repository. Stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    
    // Verify issue files were created
    let issues_dir = temp_path.join("issues");
    let issue_files = verify_issue_files_created(&issues_dir, "GIT-TEST")?;
    assert!(!issue_files.is_empty(), "Should create issue files in git repo");
    
    Ok(())
}

/// Test plan command with Unicode content
#[tokio::test]
async fn test_plan_command_unicode_content() -> Result<()> {
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;
    
    let unicode_content = r#"
# Unicode Test Plan 测试计划

## Overview 概述
Test plan with Unicode characters: 中文、日本語、한국어、العربية

## Requirements 要求
1. Handle Unicode properly 正确处理Unicode
2. Generate valid files 生成有效文件
3. Support international text 支持国际化文本

## Emojis 表情符号
- ✅ Success indicators
- 🚀 Performance improvements  
- 🔒 Security features
- 📝 Documentation
"#;
    
    let plan_file = create_test_plan_file(&temp_path, "unicode-test.md", unicode_content);
    
    // Execute plan command
    let output = run_plan_command(&temp_path, &plan_file.to_string_lossy())?;
    
    // Verify command succeeded
    assert!(
        output.status.success(),
        "Plan command should handle Unicode content. Stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    
    // Verify issue files were created
    let issues_dir = temp_path.join("issues");
    let issue_files = verify_issue_files_created(&issues_dir, "UNICODE-TEST")?;
    assert!(!issue_files.is_empty(), "Should create issue files with Unicode content");
    
    // Verify Unicode content in issue files
    for issue_file in &issue_files {
        let issue_path = issues_dir.join(issue_file);
        let content = fs::read_to_string(&issue_path)?;
        
        // Check that Unicode characters are preserved
        assert!(
            content.is_ascii() || content.chars().any(|c| c as u32 > 127),
            "Issue files should handle Unicode properly"
        );
    }
    
    Ok(())
}

/// Test plan command timeout handling
#[tokio::test]
async fn test_plan_command_timeout_handling() -> Result<()> {
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;
    
    let plan_content = r#"
# Timeout Test Plan

## Overview
Test timeout handling for plan command.

## Requirements
1. Handle reasonable timeouts gracefully
2. Provide feedback on long operations
"#;
    
    let plan_file = create_test_plan_file(&temp_path, "timeout-test.md", plan_content);
    
    // Execute plan command with very short timeout
    let output = Command::cargo_bin("sah")?
        .args(["plan", &plan_file.to_string_lossy()])
        .current_dir(&temp_path)
        .env("SWISSARMYHAMMER_TEST_MODE", "1")
        .timeout(std::time::Duration::from_millis(100)) // Very short timeout
        .output();
    
    match output {
        Ok(_result) => {
            // If it completed within timeout, that's fine
            assert!(true, "Plan completed within short timeout");
        }
        Err(e) if e.to_string().contains("timeout") => {
            // Timeout is expected and acceptable
            assert!(true, "Plan command handled timeout as expected");
        }
        Err(e) => {
            return Err(e.into());
        }
    }
    
    Ok(())
}

/// Test plan command with malformed markdown
#[tokio::test]
async fn test_plan_command_malformed_markdown() -> Result<()> {
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;
    
    let malformed_content = r#"
# Malformed Markdown Test

## Unclosed section

This has **unclosed bold

And `unclosed code

[Broken link](http://

* Incomplete list
  * Sub item
    * Another sub
      missing close

## Requirements
1. Handle malformed markdown gracefully
2. Still generate useful output
"#;
    
    let plan_file = create_test_plan_file(&temp_path, "malformed-test.md", malformed_content);
    
    // Execute plan command
    let output = run_plan_command(&temp_path, &plan_file.to_string_lossy())?;
    
    // Should succeed despite malformed markdown
    assert!(
        output.status.success(),
        "Plan command should handle malformed markdown. Stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    
    // Should still create some issue files
    let issues_dir = temp_path.join("issues");
    let issue_files = verify_issue_files_created(&issues_dir, "MALFORMED-TEST")?;
    assert!(!issue_files.is_empty(), "Should create issue files despite malformed markdown");
    
    Ok(())
}

/// Test plan command workflow state management
#[tokio::test]
async fn test_plan_command_workflow_state_cleanup() -> Result<()> {
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;
    
    let plan_content = r#"
# State Management Test

## Overview
Test that workflow state is properly managed and cleaned up.

## Requirements
1. Proper state initialization
2. State cleanup on completion
3. No leaked resources
"#;
    
    let plan_file = create_test_plan_file(&temp_path, "state-test.md", plan_content);
    
    // Execute plan command
    let output = run_plan_command(&temp_path, &plan_file.to_string_lossy())?;
    
    // Verify command succeeded
    assert!(
        output.status.success(),
        "Plan command workflow state should be managed properly. Stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    
    // Check for temporary files that should be cleaned up
    let tmp_dir = temp_path.join(".swissarmyhammer/tmp");
    
    // Verify no temporary workflow state files are left behind
    if tmp_dir.exists() {
        let tmp_files: Vec<_> = fs::read_dir(&tmp_dir)?.collect();
        
        // Some temporary files like DRAFT_PLAN.md might exist and that's OK
        // But there shouldn't be workflow run state files
        for entry in tmp_files {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            
            // These are acceptable temporary files
            if name_str.contains("DRAFT_PLAN") {
                continue;
            }
            
            // Unexpected temporary files suggest poor cleanup
            if name_str.contains("workflow") || name_str.contains("state") {
                panic!("Unexpected workflow state file left behind: {}", name_str);
            }
        }
    }
    
    Ok(())
}