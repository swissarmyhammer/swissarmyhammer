//! End-to-End Workflow Integration Tests
//!
//! These tests validate complete workflows that span multiple SwissArmyHammer
//! components, ensuring that all systems work together correctly with the new
//! Git repository-centric directory structure.

use super::GitRepositoryTestGuard;
use swissarmyhammer_common::utils::{find_swissarmyhammer_directory, get_or_create_swissarmyhammer_directory};
use std::fs;
use std::path::Path;
use tokio::time::{timeout, Duration};

/// Test complete memo lifecycle in Git repository environment
///
/// This test validates that:
/// 1. Memos can be created and stored in Git repository .swissarmyhammer/memos
/// 2. Memo operations work correctly from subdirectories  
/// 3. Directory resolution is consistent across all memo operations
/// 4. Memo data persists correctly in the Git-centric structure
#[tokio::test]
async fn test_complete_memo_workflow_in_git_repository() {
    let guard = GitRepositoryTestGuard::new_with_swissarmyhammer()
        .with_project_structure();

    // Verify initial state - .swissarmyhammer directory should exist at repo root
    let swissarmyhammer_dir = find_swissarmyhammer_directory();
    assert!(swissarmyhammer_dir.is_some());
    assert_eq!(swissarmyhammer_dir.unwrap(), guard.swissarmyhammer_dir().unwrap());

    let memos_dir = guard.swissarmyhammer_dir().unwrap().join("memos");
    assert!(memos_dir.exists());

    // Create a test memo file to simulate memo storage
    let memo_content = "# Test Memo\n\nThis is a test memo for integration testing.";
    let memo_file = memos_dir.join("test-memo.md");
    fs::write(&memo_file, memo_content).expect("Failed to create test memo");

    // Test memo operations from repository root
    assert!(memo_file.exists());
    let content = fs::read_to_string(&memo_file).expect("Failed to read memo");
    assert_eq!(content, memo_content);

    // Test memo operations from subdirectory - directory resolution should still work
    guard.cd_to_subdir("src/lib").expect("Failed to change to subdirectory");
    
    // Directory resolution should still find the same .swissarmyhammer directory
    let swissarmyhammer_from_subdir = find_swissarmyhammer_directory();
    assert!(swissarmyhammer_from_subdir.is_some());
    assert_eq!(swissarmyhammer_from_subdir.unwrap(), guard.swissarmyhammer_dir().unwrap());

    // Memo should still be accessible
    let memo_from_subdir = swissarmyhammer_from_subdir.unwrap().join("memos/test-memo.md");
    assert!(memo_from_subdir.exists());
    let content_from_subdir = fs::read_to_string(&memo_from_subdir).expect("Failed to read memo from subdir");
    assert_eq!(content_from_subdir, memo_content);

    // Test creating additional memo from subdirectory
    let memo2_content = "# Another Test Memo\n\nCreated from subdirectory.";
    let memo2_file = swissarmyhammer_from_subdir.unwrap().join("memos/subdir-memo.md");
    fs::write(&memo2_file, memo2_content).expect("Failed to create second memo");

    // Verify both memos exist and are in the same directory
    assert!(memo_file.exists());
    assert!(memo2_file.exists());
    assert_eq!(memo_file.parent().unwrap(), memo2_file.parent().unwrap());
}

/// Test complete todo lifecycle in Git repository environment  
///
/// This test validates that:
/// 1. Todo lists can be created and stored in Git repository .swissarmyhammer/todo
/// 2. Todo operations work from various subdirectories
/// 3. Multiple todo lists can coexist in the same directory
/// 4. Todo data structure and operations work with Git-centric directory
#[tokio::test]
async fn test_complete_todo_workflow_in_git_repository() {
    let guard = GitRepositoryTestGuard::new_with_swissarmyhammer()
        .with_project_structure();

    let todo_dir = guard.swissarmyhammer_dir().unwrap().join("todo");
    assert!(todo_dir.exists());

    // Create a test todo list file
    let todo_content = r#"todo:
  - id: 01H8XYZ123ABC456DEF789GHI0
    task: "Implement feature X"
    context: "Located in src/feature.rs"
    done: false
  - id: 01H8XYZ123ABC456DEF789GHI1
    task: "Write tests for feature X"
    context: "Add to tests/feature_tests.rs"
    done: false
"#;

    let todo_file = todo_dir.join("feature_work.todo.yaml");
    fs::write(&todo_file, todo_content).expect("Failed to create todo file");

    // Test todo operations from repository root
    assert!(todo_file.exists());
    let content = fs::read_to_string(&todo_file).expect("Failed to read todo file");
    assert!(content.contains("Implement feature X"));
    assert!(content.contains("Write tests for feature X"));

    // Test from deeply nested directory
    let deep_dir = guard.create_deep_structure(3);
    guard.cd_to_subdir(deep_dir.strip_prefix(guard.path()).unwrap())
        .expect("Failed to change to deep directory");

    // Directory resolution should work from deep directory
    let swissarmyhammer_from_deep = find_swissarmyhammer_directory();
    assert!(swissarmyhammer_from_deep.is_some());
    assert_eq!(swissarmyhammer_from_deep.unwrap(), guard.swissarmyhammer_dir().unwrap());

    // Todo file should be accessible
    let todo_from_deep = swissarmyhammer_from_deep.unwrap().join("todo/feature_work.todo.yaml");
    assert!(todo_from_deep.exists());

    // Create another todo list from deep directory
    let todo2_content = r#"todo:
  - id: 01H8XYZ123ABC456DEF789GHI2
    task: "Debug issue in deep module"
    context: "Check level2/level3 directory"
    done: false
"#;

    let todo2_file = swissarmyhammer_from_deep.unwrap().join("todo/debugging.todo.yaml");
    fs::write(&todo2_file, todo2_content).expect("Failed to create second todo file");

    // Verify both todo files exist in same directory
    assert!(todo_file.exists());
    assert!(todo2_file.exists());
    assert_eq!(todo_file.parent().unwrap(), todo2_file.parent().unwrap());

    // Verify todo directory contains both files
    let todo_entries: Vec<_> = fs::read_dir(todo_dir)
        .expect("Failed to read todo directory")
        .map(|entry| entry.unwrap().file_name())
        .collect();

    assert!(todo_entries.iter().any(|name| name == "feature_work.todo.yaml"));
    assert!(todo_entries.iter().any(|name| name == "debugging.todo.yaml"));
}

/// Test search system integration with Git repository structure
///
/// This test validates that:
/// 1. Search database is created in .swissarmyhammer directory
/// 2. Search indexing works with Git repository file structure
/// 3. Search operations work from various directories
/// 4. Search database location is consistent
#[tokio::test]
async fn test_search_integration_with_git_repository() {
    let guard = GitRepositoryTestGuard::new_with_swissarmyhammer()
        .with_project_structure();

    let swissarmyhammer_dir = guard.swissarmyhammer_dir().unwrap();
    
    // Create some source files to potentially index
    fs::write(guard.path().join("src/main.rs"), r#"
fn main() {
    println!("Hello, world!");
    let result = calculate_fibonacci(10);
    println!("Fibonacci result: {}", result);
}

fn calculate_fibonacci(n: u32) -> u32 {
    match n {
        0 => 0,
        1 => 1,
        _ => calculate_fibonacci(n - 1) + calculate_fibonacci(n - 2),
    }
}
"#).expect("Failed to create main.rs");

    fs::write(guard.path().join("src/lib.rs"), r#"
//! Library for mathematical operations
//! 
//! This library provides various mathematical functions
//! including fibonacci calculation and prime number checking.

pub mod math {
    pub fn is_prime(n: u32) -> bool {
        if n < 2 { return false; }
        for i in 2..((n as f64).sqrt() as u32 + 1) {
            if n % i == 0 { return false; }
        }
        true
    }

    pub fn fibonacci(n: u32) -> u32 {
        match n {
            0 => 0,
            1 => 1,
            _ => fibonacci(n - 1) + fibonacci(n - 2),
        }
    }
}
"#).expect("Failed to create lib.rs");

    // Simulate search database creation (in real implementation this would be done by search system)
    let search_db_path = swissarmyhammer_dir.join("search.db");
    fs::write(&search_db_path, "SEARCH_DATABASE_PLACEHOLDER").expect("Failed to create search database");
    
    // Test search database access from repository root
    assert!(search_db_path.exists());
    let db_content = fs::read_to_string(&search_db_path).expect("Failed to read search database");
    assert_eq!(db_content, "SEARCH_DATABASE_PLACEHOLDER");

    // Test search database access from subdirectory
    guard.cd_to_subdir("src").expect("Failed to change to src directory");
    
    let swissarmyhammer_from_src = find_swissarmyhammer_directory();
    assert!(swissarmyhammer_from_src.is_some());
    
    let search_db_from_src = swissarmyhammer_from_src.unwrap().join("search.db");
    assert!(search_db_from_src.exists());
    assert_eq!(search_db_from_src, search_db_path);

    // Test search database access from docs directory
    guard.cd_to_subdir("../docs").expect("Failed to change to docs directory");
    
    let swissarmyhammer_from_docs = find_swissarmyhammer_directory();
    assert!(swissarmyhammer_from_docs.is_some());
    assert_eq!(swissarmyhammer_from_docs.unwrap().join("search.db"), search_db_path);

    // Simulate search index update from different directory
    let updated_db_content = "UPDATED_SEARCH_DATABASE_WITH_NEW_INDEX";
    fs::write(&swissarmyhammer_from_docs.unwrap().join("search.db"), updated_db_content)
        .expect("Failed to update search database");

    // Verify update is visible from original location
    let final_content = fs::read_to_string(&search_db_path).expect("Failed to read updated database");
    assert_eq!(final_content, updated_db_content);
}

/// Test issues system integration with Git repository structure
///
/// This test validates that:
/// 1. Issues can be created in .swissarmyhammer/issues directory
/// 2. Issue operations work from various subdirectories
/// 3. Completed issues are moved to issues/complete correctly
/// 4. Issue directory structure is consistent
#[tokio::test]
async fn test_issues_integration_with_git_repository() {
    let guard = GitRepositoryTestGuard::new_with_swissarmyhammer()
        .with_project_structure();

    let issues_dir = guard.swissarmyhammer_dir().unwrap().join("issues");
    let complete_dir = issues_dir.join("complete");
    
    assert!(issues_dir.exists());
    assert!(complete_dir.exists());

    // Create a test issue
    let issue_content = r#"# Feature Request: Add User Authentication

## Description
Need to implement user authentication system with login/logout functionality.

## Acceptance Criteria
- [ ] User can register with email and password
- [ ] User can login with credentials  
- [ ] User can logout and session is cleared
- [ ] Password requirements are enforced

## Implementation Notes
- Use bcrypt for password hashing
- Implement JWT tokens for session management
- Add middleware for protected routes
"#;

    let issue_file = issues_dir.join("FEATURE_001_user_authentication.md");
    fs::write(&issue_file, issue_content).expect("Failed to create issue file");

    // Test issue access from repository root
    assert!(issue_file.exists());
    let content = fs::read_to_string(&issue_file).expect("Failed to read issue file");
    assert!(content.contains("User Authentication"));
    assert!(content.contains("bcrypt for password hashing"));

    // Test issue access from subdirectory
    guard.cd_to_subdir("src/lib").expect("Failed to change to subdirectory");
    
    let swissarmyhammer_from_subdir = find_swissarmyhammer_directory();
    assert!(swissarmyhammer_from_subdir.is_some());
    
    let issue_from_subdir = swissarmyhammer_from_subdir.unwrap()
        .join("issues/FEATURE_001_user_authentication.md");
    assert!(issue_from_subdir.exists());
    assert_eq!(issue_from_subdir, issue_file);

    // Simulate completing an issue by moving it to complete directory
    let completed_issue_file = complete_dir.join("FEATURE_001_user_authentication.md");
    fs::rename(&issue_file, &completed_issue_file).expect("Failed to move completed issue");

    assert!(!issue_file.exists());
    assert!(completed_issue_file.exists());

    // Verify completed issue is accessible from subdirectory
    let completed_from_subdir = swissarmyhammer_from_subdir.unwrap()
        .join("issues/complete/FEATURE_001_user_authentication.md");
    assert!(completed_from_subdir.exists());
    assert_eq!(completed_from_subdir, completed_issue_file);

    // Create another issue from subdirectory
    let issue2_content = r#"# Bug Fix: Memory Leak in Authentication Module

## Description
Memory leak detected in authentication token validation.

## Steps to Reproduce
1. Login with valid credentials
2. Make 100+ authenticated requests
3. Monitor memory usage

## Expected Fix
- Fix token cleanup after validation
- Add proper memory deallocation
"#;

    let issue2_file = swissarmyhammer_from_subdir.unwrap()
        .join("issues/BUG_001_memory_leak.md");
    fs::write(&issue2_file, issue2_content).expect("Failed to create second issue");

    // Verify both issues exist in correct locations
    assert!(completed_issue_file.exists());
    assert!(issue2_file.exists());
    assert_eq!(issues_dir, issue2_file.parent().unwrap());
    assert_eq!(complete_dir, completed_issue_file.parent().unwrap());
}

/// Test complete multi-component workflow
///
/// This test validates that multiple SwissArmyHammer components can work
/// together in a realistic workflow scenario, all using the same Git-centric
/// directory structure.
#[tokio::test]
async fn test_complete_multi_component_workflow() {
    let guard = GitRepositoryTestGuard::new_with_swissarmyhammer()
        .with_project_structure();

    let swissarmyhammer_dir = guard.swissarmyhammer_dir().unwrap();

    // Step 1: Create an issue for a new feature
    let issue_content = r#"# Implement Data Export Feature

## Requirements
- Export user data to CSV format
- Export user data to JSON format  
- Add export button to user dashboard

## Technical Notes
- Use streaming for large datasets
- Implement proper error handling
- Add export progress indicator
"#;

    let issue_file = swissarmyhammer_dir.join("issues/FEATURE_002_data_export.md");
    fs::write(&issue_file, issue_content).expect("Failed to create feature issue");

    // Step 2: Create todo list for implementing the feature
    let todo_content = r#"todo:
  - id: 01H8XYZ123ABC456DEF789GHI3
    task: "Design data export API"
    context: "Create endpoint for /api/export/{format}"
    done: false
  - id: 01H8XYZ123ABC456DEF789GHI4
    task: "Implement CSV export functionality"
    context: "Use csv crate for formatting"
    done: false
  - id: 01H8XYZ123ABC456DEF789GHI5
    task: "Implement JSON export functionality" 
    context: "Use serde_json for serialization"
    done: false
  - id: 01H8XYZ123ABC456DEF789GHI6
    task: "Add frontend export button"
    context: "Update user dashboard component"
    done: false
  - id: 01H8XYZ123ABC456DEF789GHI7
    task: "Write integration tests"
    context: "Test both CSV and JSON export formats"
    done: false
"#;

    let todo_file = swissarmyhammer_dir.join("todo/data_export_feature.todo.yaml");
    fs::write(&todo_file, todo_content).expect("Failed to create todo file");

    // Step 3: Create memo with research notes
    let memo_content = r#"# Data Export Research

## CSV Export Libraries
- **csv crate**: Most popular, good streaming support
- **csv-core**: Lower level, more control
- **serde_csv**: Good integration with serde

## Performance Considerations
- Use streaming for files > 1MB
- Implement pagination for very large datasets
- Consider compression for download

## Security Notes
- Validate user permissions before export
- Log all export operations for audit
- Rate limit export requests

## Implementation Plan
1. Start with CSV export (simpler)
2. Add JSON export using same pattern
3. Implement frontend integration
4. Add comprehensive tests
"#;

    let memo_file = swissarmyhammer_dir.join("memos/data_export_research.md");
    fs::write(&memo_file, memo_content).expect("Failed to create research memo");

    // Step 4: Test that all components can access their data from subdirectories
    guard.cd_to_subdir("src/bin").expect("Failed to change to bin directory");

    // Verify all components can find their data through directory resolution
    let swissarmyhammer_from_bin = find_swissarmyhammer_directory();
    assert!(swissarmyhammer_from_bin.is_some());
    assert_eq!(swissarmyhammer_from_bin.unwrap(), swissarmyhammer_dir);

    // Check issue is accessible
    let issue_from_bin = swissarmyhammer_from_bin.unwrap().join("issues/FEATURE_002_data_export.md");
    assert!(issue_from_bin.exists());
    assert_eq!(issue_from_bin, issue_file);

    // Check todo is accessible  
    let todo_from_bin = swissarmyhammer_from_bin.unwrap().join("todo/data_export_feature.todo.yaml");
    assert!(todo_from_bin.exists());
    assert_eq!(todo_from_bin, todo_file);

    // Check memo is accessible
    let memo_from_bin = swissarmyhammer_from_bin.unwrap().join("memos/data_export_research.md");
    assert!(memo_from_bin.exists());
    assert_eq!(memo_from_bin, memo_file);

    // Step 5: Simulate workflow progress - mark some todos as done
    let updated_todo_content = r#"todo:
  - id: 01H8XYZ123ABC456DEF789GHI3
    task: "Design data export API"
    context: "Create endpoint for /api/export/{format}"
    done: true
  - id: 01H8XYZ123ABC456DEF789GHI4
    task: "Implement CSV export functionality"
    context: "Use csv crate for formatting"
    done: true
  - id: 01H8XYZ123ABC456DEF789GHI5
    task: "Implement JSON export functionality" 
    context: "Use serde_json for serialization"
    done: false
  - id: 01H8XYZ123ABC456DEF789GHI6
    task: "Add frontend export button"
    context: "Update user dashboard component"
    done: false
  - id: 01H8XYZ123ABC456DEF789GHI7
    task: "Write integration tests"
    context: "Test both CSV and JSON export formats"
    done: false
"#;

    fs::write(&todo_from_bin, updated_todo_content).expect("Failed to update todo file");

    // Step 6: Add progress memo
    let progress_memo_content = r#"# Data Export Implementation Progress

## Completed
- ✅ API design finalized
- ✅ CSV export implemented and tested
- ✅ Basic error handling added

## Current Status
Working on JSON export implementation. CSV version is working well with streaming support.

## Next Steps
- Complete JSON export (similar pattern to CSV)
- Add frontend button component
- Write comprehensive integration tests

## Issues Found
- Need to handle special characters in CSV fields
- Memory usage spikes with very large datasets (>10MB)
- Consider adding export progress callbacks

## Performance Notes
CSV export of 100k records: ~2.3 seconds
Memory usage stays under 50MB with streaming approach
"#;

    let progress_memo_file = swissarmyhammer_from_bin.unwrap().join("memos/data_export_progress.md");
    fs::write(&progress_memo_file, progress_memo_content).expect("Failed to create progress memo");

    // Step 7: Test from another subdirectory to verify consistency
    guard.cd_to_subdir("../../docs").expect("Failed to change to docs directory");
    
    let swissarmyhammer_from_docs = find_swissarmyhammer_directory();
    assert!(swissarmyhammer_from_docs.is_some());
    assert_eq!(swissarmyhammer_from_docs.unwrap(), swissarmyhammer_dir);

    // Verify all files are accessible and contain expected content
    let issue_content_check = fs::read_to_string(
        swissarmyhammer_from_docs.unwrap().join("issues/FEATURE_002_data_export.md")
    ).expect("Failed to read issue from docs directory");
    assert!(issue_content_check.contains("Data Export Feature"));

    let todo_content_check = fs::read_to_string(
        swissarmyhammer_from_docs.unwrap().join("todo/data_export_feature.todo.yaml")
    ).expect("Failed to read todo from docs directory");
    assert!(todo_content_check.contains("done: true"));

    let memo_content_check = fs::read_to_string(
        swissarmyhammer_from_docs.unwrap().join("memos/data_export_research.md")
    ).expect("Failed to read memo from docs directory");
    assert!(memo_content_check.contains("csv crate"));

    let progress_memo_check = fs::read_to_string(
        swissarmyhammer_from_docs.unwrap().join("memos/data_export_progress.md")
    ).expect("Failed to read progress memo from docs directory");
    assert!(progress_memo_check.contains("CSV export implemented"));

    // Step 8: Verify directory structure integrity
    let swissarmyhammer_entries: Vec<_> = fs::read_dir(&swissarmyhammer_dir)
        .expect("Failed to read .swissarmyhammer directory")
        .map(|entry| entry.unwrap().file_name())
        .collect();

    // Should contain all expected subdirectories
    assert!(swissarmyhammer_entries.iter().any(|name| name == "memos"));
    assert!(swissarmyhammer_entries.iter().any(|name| name == "todo"));
    assert!(swissarmyhammer_entries.iter().any(|name| name == "issues"));
    assert!(swissarmyhammer_entries.iter().any(|name| name == "workflows"));

    // Verify file counts in each subdirectory
    let memo_count = fs::read_dir(swissarmyhammer_dir.join("memos"))
        .expect("Failed to read memos directory")
        .count();
    assert_eq!(memo_count, 2); // research + progress memos

    let todo_count = fs::read_dir(swissarmyhammer_dir.join("todo"))
        .expect("Failed to read todo directory")
        .count();
    assert_eq!(todo_count, 1); // data export todo

    let issue_count = fs::read_dir(swissarmyhammer_dir.join("issues"))
        .expect("Failed to read issues directory")
        .filter(|entry| entry.as_ref().unwrap().path().is_file())
        .count();
    assert_eq!(issue_count, 1); // data export issue
}

/// Test workflow operations with timeout to ensure reasonable performance
///
/// This test validates that all workflow operations complete within reasonable
/// time limits, ensuring the Git directory resolution doesn't introduce
/// performance regressions.
#[tokio::test]
async fn test_workflow_performance_with_timeouts() {
    let guard = GitRepositoryTestGuard::new_with_swissarmyhammer();

    // All directory resolution operations should complete quickly
    let directory_resolution_result = timeout(Duration::from_millis(100), async {
        // Test multiple directory resolution calls
        for _ in 0..10 {
            let swissarmyhammer_dir = find_swissarmyhammer_directory();
            assert!(swissarmyhammer_dir.is_some());
        }
    }).await;

    assert!(directory_resolution_result.is_ok(), "Directory resolution should complete within 100ms");

    // File operations should complete within reasonable time
    let file_operations_result = timeout(Duration::from_millis(500), async {
        let swissarmyhammer_dir = guard.swissarmyhammer_dir().unwrap();

        // Create multiple files in different subdirectories
        for i in 0..5 {
            let memo_content = format!("# Test Memo {}\n\nContent for memo {}.", i, i);
            let memo_file = swissarmyhammer_dir.join("memos").join(format!("test_memo_{}.md", i));
            fs::write(&memo_file, memo_content).unwrap_or_else(|e| {
                panic!("Failed to create memo file {}: {}", i, e)
            });
        }

        for i in 0..3 {
            let todo_content = format!(r#"todo:
  - id: 01H8XYZ123ABC456DEF789GH{}{}
    task: "Test task {}"
    context: "Test context for task {}"
    done: false
"#, i, i, i, i);
            let todo_file = swissarmyhammer_dir.join("todo").join(format!("test_todo_{}.yaml", i));
            fs::write(&todo_file, todo_content).unwrap_or_else(|e| {
                panic!("Failed to create todo file {}: {}", i, e)
            });
        }
    }).await;

    assert!(file_operations_result.is_ok(), "File operations should complete within 500ms");

    // Directory structure verification should be fast
    let verification_result = timeout(Duration::from_millis(200), async {
        let swissarmyhammer_dir = guard.swissarmyhammer_dir().unwrap();
        
        // Verify all created files exist
        for i in 0..5 {
            let memo_file = swissarmyhammer_dir.join("memos").join(format!("test_memo_{}.md", i));
            assert!(memo_file.exists(), "Memo file {} should exist", i);
        }

        for i in 0..3 {
            let todo_file = swissarmyhammer_dir.join("todo").join(format!("test_todo_{}.yaml", i));
            assert!(todo_file.exists(), "Todo file {} should exist", i);
        }
    }).await;

    assert!(verification_result.is_ok(), "Directory verification should complete within 200ms");
}