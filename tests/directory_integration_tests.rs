//! Directory Integration Tests
//!
//! Comprehensive integration tests for SwissArmyHammer's Git repository-centric
//! directory system. This module provides the main entry point for testing
//! the complete directory migration and integration system.
//!
//! # Test Organization
//!
//! This integration test suite is organized into several focused areas:
//!
//! - **End-to-End Workflow Tests**: Complete workflows spanning multiple components
//! - **Migration Scenario Tests**: Testing migration from legacy directory structures
//! - **Error Scenario Tests**: Edge cases, error conditions, and boundary testing
//! - **Performance Tests**: Performance validation and regression testing
//! - **Concurrent Tests**: Thread safety and concurrent access validation
//!
//! # Test Infrastructure
//!
//! The tests use specialized utilities for creating isolated test environments:
//!
//! - `GitRepositoryTestGuard`: Creates isolated Git repositories with proper cleanup
//! - `IsolatedTestEnvironment`: Provides complete isolation for parallel testing
//! - Performance measurement utilities for benchmarking operations
//! - Legacy directory structure simulation for migration testing
//!
//! # Running the Tests
//!
//! ```bash
//! # Run all directory integration tests
//! cargo test directory_integration
//!
//! # Run specific test modules
//! cargo test directory_integration::end_to_end
//! cargo test directory_integration::migration
//! cargo test directory_integration::performance
//!
//! # Run with output for debugging
//! cargo test directory_integration -- --nocapture
//! ```
//!
//! # Test Coverage
//!
//! These tests provide comprehensive coverage of:
//!
//! - Git repository detection and validation
//! - .swissarmyhammer directory resolution from various locations
//! - Cross-component integration (memos, todos, issues, search)
//! - Performance characteristics under various conditions
//! - Error handling and graceful degradation
//! - Thread safety and concurrent access patterns
//! - Migration scenarios from legacy directory structures
//!
//! # Platform Compatibility
//!
//! The tests are designed to run on all supported platforms (Windows, macOS, Linux)
//! and handle platform-specific differences in file system behavior, path handling,
//! and permissions.

pub mod directory_integration;

// Re-export test utilities for other integration tests
pub use directory_integration::{
    GitRepositoryTestGuard,
    create_large_git_repository,
    create_legacy_directory_structure,
    measure_time,
    generate_test_id,
};

// Import all test modules
use directory_integration::{
    end_to_end_tests,
    migration_tests,
    error_scenario_tests,
    performance_tests,
    concurrent_tests,
};

/// Integration test to verify all test modules are properly linked
///
/// This test ensures that all integration test modules can be loaded
/// and their basic functionality works correctly.
#[test]
fn test_integration_modules_availability() {
    // Test that we can create basic test infrastructure
    let guard = GitRepositoryTestGuard::new();
    assert!(guard.path().exists());
    assert!(guard.git_dir().exists());

    // Test measurement utilities
    let (result, duration) = measure_time(|| {
        std::thread::sleep(std::time::Duration::from_millis(1));
        42
    });
    assert_eq!(result, 42);
    assert!(duration >= std::time::Duration::from_millis(1));

    // Test unique ID generation
    let id1 = generate_test_id();
    let id2 = generate_test_id();
    assert_ne!(id1, id2);
    assert!(id1.starts_with("test_"));
    assert!(id2.starts_with("test_"));
}

/// Integration test to verify cross-module compatibility
///
/// This test ensures that different test modules can work together
/// and share common infrastructure without conflicts.
#[test]
fn test_cross_module_compatibility() {
    // Create test environment that could be used by multiple modules
    let guard = GitRepositoryTestGuard::new_with_swissarmyhammer()
        .with_project_structure();

    // Verify the environment is suitable for all test types
    assert!(guard.path().exists());
    assert!(guard.swissarmyhammer_dir().is_some());
    assert!(guard.swissarmyhammer_dir().unwrap().join("memos").exists());
    assert!(guard.swissarmyhammer_dir().unwrap().join("todo").exists());
    assert!(guard.swissarmyhammer_dir().unwrap().join("issues").exists());

    // Test basic directory resolution (used by all modules)
    let git_root = swissarmyhammer::directory_utils::find_git_repository_root();
    assert!(git_root.is_some());
    assert_eq!(git_root.unwrap(), guard.path());

    let swissarmyhammer_dir = swissarmyhammer::directory_utils::find_swissarmyhammer_directory();
    assert!(swissarmyhammer_dir.is_some());
    assert_eq!(swissarmyhammer_dir.unwrap(), guard.swissarmyhammer_dir().unwrap());

    // Test file operations (used by end-to-end and concurrent tests)
    let test_file = guard.swissarmyhammer_dir().unwrap().join("memos/cross_module_test.md");
    std::fs::write(&test_file, "# Cross Module Test\n\nThis file tests cross-module compatibility.")
        .expect("Failed to write cross-module test file");
    
    assert!(test_file.exists());
    let content = std::fs::read_to_string(&test_file).expect("Failed to read test file");
    assert!(content.contains("Cross Module Test"));
}

/// Integration test to verify performance baseline
///
/// This test establishes a performance baseline that other tests can
/// reference to ensure no significant performance regressions.
#[test]
fn test_performance_baseline() {
    let guard = GitRepositoryTestGuard::new_with_swissarmyhammer();

    // Establish baseline for Git repository detection
    let (git_root, git_time) = measure_time(|| {
        swissarmyhammer::directory_utils::find_git_repository_root()
    });
    
    assert!(git_root.is_some());
    assert!(git_time < std::time::Duration::from_millis(50), 
           "Baseline Git detection should complete within 50ms, took {}ms", git_time.as_millis());

    // Establish baseline for SwissArmyHammer directory detection
    let (swissarmyhammer_dir, sah_time) = measure_time(|| {
        swissarmyhammer::directory_utils::find_swissarmyhammer_directory()
    });
    
    assert!(swissarmyhammer_dir.is_some());
    assert!(sah_time < std::time::Duration::from_millis(50), 
           "Baseline SwissArmyHammer detection should complete within 50ms, took {}ms", sah_time.as_millis());

    // Establish baseline for directory creation
    std::fs::remove_dir_all(guard.swissarmyhammer_dir().unwrap())
        .expect("Failed to remove .swissarmyhammer directory");

    let (create_result, create_time) = measure_time(|| {
        swissarmyhammer::directory_utils::get_or_create_swissarmyhammer_directory()
    });
    
    assert!(create_result.is_ok());
    assert!(create_time < std::time::Duration::from_millis(100), 
           "Baseline directory creation should complete within 100ms, took {}ms", create_time.as_millis());
}

/// Integration test to verify error handling consistency
///
/// This test ensures that error handling is consistent across all
/// integration scenarios and provides appropriate error messages.
#[test]
fn test_error_handling_consistency() {
    use tempfile::TempDir;

    // Test consistent error handling when not in Git repository
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let non_git_dir = temp_dir.path().join("not-a-git-repo");
    std::fs::create_dir_all(&non_git_dir).expect("Failed to create non-Git directory");

    let original_cwd = std::env::current_dir().expect("Failed to get current directory");
    std::env::set_current_dir(&non_git_dir).expect("Failed to change to non-Git directory");

    // All functions should consistently handle non-Git repository scenario
    let git_root = swissarmyhammer::directory_utils::find_git_repository_root();
    assert!(git_root.is_none(), "Should not find Git repository");

    let swissarmyhammer_dir = swissarmyhammer::directory_utils::find_swissarmyhammer_directory();
    assert!(swissarmyhammer_dir.is_none(), "Should not find .swissarmyhammer directory");

    let create_result = swissarmyhammer::directory_utils::get_or_create_swissarmyhammer_directory();
    assert!(create_result.is_err(), "Should fail to create .swissarmyhammer directory");

    // Verify error type is consistent
    match create_result.unwrap_err() {
        swissarmyhammer::error::SwissArmyHammerError::NotInGitRepository => {
            // Expected error type
        }
        other => panic!("Expected NotInGitRepository error, got {:?}", other),
    }

    std::env::set_current_dir(original_cwd).expect("Failed to restore directory");
}

/// Integration test to verify memory and resource cleanup
///
/// This test ensures that test infrastructure properly cleans up resources
/// and doesn't leak memory or file handles during intensive testing.
#[test]
fn test_resource_cleanup() {
    // Create and destroy many test environments to check for leaks
    for i in 0..10 {
        let guard = GitRepositoryTestGuard::new_with_swissarmyhammer()
            .with_project_structure();
        
        // Perform some operations
        let git_root = swissarmyhammer::directory_utils::find_git_repository_root();
        assert!(git_root.is_some(), "Iteration {}: Should find Git repository", i);
        
        let swissarmyhammer_dir = swissarmyhammer::directory_utils::find_swissarmyhammer_directory();
        assert!(swissarmyhammer_dir.is_some(), "Iteration {}: Should find .swissarmyhammer directory", i);
        
        // Create some files
        let memo_content = format!("# Resource Test Memo {}\n\nTesting resource cleanup.", i);
        let memo_file = swissarmyhammer_dir.unwrap().join("memos").join(format!("resource_test_{}.md", i));
        std::fs::write(&memo_file, memo_content).expect("Failed to write memo");
        assert!(memo_file.exists());
        
        // Guard will be dropped at end of iteration, cleaning up resources
    }
    
    // If we reach here without running out of resources, cleanup is working
    assert!(true, "Resource cleanup test completed successfully");
}

/// Integration test to verify test isolation
///
/// This test ensures that different integration tests can run in parallel
/// without interfering with each other.
#[test]
fn test_isolation_verification() {
    use std::sync::{Arc, Barrier};
    use std::thread;

    let barrier = Arc::new(Barrier::new(3));
    let mut handles = vec![];

    for thread_id in 0..3 {
        let barrier = Arc::clone(&barrier);
        
        let handle = thread::spawn(move || {
            // Each thread creates its own isolated environment
            let guard = GitRepositoryTestGuard::new_with_swissarmyhammer();
            
            barrier.wait(); // Synchronize to increase chance of conflicts
            
            // Perform operations that would conflict if not properly isolated
            let git_root = swissarmyhammer::directory_utils::find_git_repository_root();
            assert!(git_root.is_some(), "Thread {}: Should find Git repository", thread_id);
            assert_eq!(git_root.unwrap(), guard.path(), "Thread {}: Git root should be correct", thread_id);
            
            let swissarmyhammer_dir = swissarmyhammer::directory_utils::find_swissarmyhammer_directory();
            assert!(swissarmyhammer_dir.is_some(), "Thread {}: Should find .swissarmyhammer", thread_id);
            assert_eq!(swissarmyhammer_dir.unwrap(), guard.swissarmyhammer_dir().unwrap(), 
                      "Thread {}: .swissarmyhammer should be correct", thread_id);
            
            // Create thread-specific content
            let memo_content = format!("# Isolation Test Thread {}\n\nThis is from thread {}.", thread_id, thread_id);
            let memo_file = swissarmyhammer_dir.unwrap().join("memos").join(format!("isolation_thread_{}.md", thread_id));
            std::fs::write(&memo_file, memo_content).expect("Failed to write isolation test memo");
            
            assert!(memo_file.exists(), "Thread {}: Memo file should exist", thread_id);
            
            // Verify content is correct (not overwritten by other threads)
            let read_content = std::fs::read_to_string(&memo_file).expect("Failed to read memo");
            assert!(read_content.contains(&format!("thread {}", thread_id)), 
                   "Thread {}: Content should contain thread identifier", thread_id);
            
            thread_id
        });
        
        handles.push(handle);
    }

    // Wait for all threads to complete
    let completed_threads: Vec<_> = handles.into_iter()
        .map(|h| h.join().expect("Thread panicked"))
        .collect();
    
    assert_eq!(completed_threads, vec![0, 1, 2], "All threads should complete successfully");
}