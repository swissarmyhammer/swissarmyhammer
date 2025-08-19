# Comprehensive Unit Tests for Core Infrastructure

## Overview
Add comprehensive unit tests for the core infrastructure changes from steps 000284-000288, ensuring all directory detection, migration detection, and storage behavior works correctly.

Refer to /Users/wballard/github/sah-issues/ideas/move_issues.md

## Current State
Core infrastructure changes have been implemented but need comprehensive testing to verify behavior across all scenarios.

## Target Implementation

### Core Storage Tests
```rust
#[cfg(test)]
mod filesystem_storage_tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_new_default_with_swissarmyhammer_directory() {
        let temp_dir = TempDir::new().unwrap();
        let swissarmyhammer_dir = temp_dir.path().join(".swissarmyhammer");
        std::fs::create_dir_all(&swissarmyhammer_dir).unwrap();
        
        std::env::set_current_dir(temp_dir.path()).unwrap();
        
        let storage = FileSystemIssueStorage::new_default().unwrap();
        let expected_path = temp_dir.path().join(".swissarmyhammer/issues");
        
        assert_eq!(storage.directory_path(), expected_path);
    }
    
    #[test]
    fn test_new_default_without_swissarmyhammer_directory() {
        let temp_dir = TempDir::new().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();
        
        let storage = FileSystemIssueStorage::new_default().unwrap();
        let expected_path = temp_dir.path().join("issues");
        
        assert_eq!(storage.directory_path(), expected_path);
    }
    
    #[test]
    fn test_default_directory_detection() {
        let temp_dir = TempDir::new().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();
        
        // Test without .swissarmyhammer
        let dir = FileSystemIssueStorage::default_directory().unwrap();
        assert_eq!(dir, temp_dir.path().join("issues"));
        
        // Create .swissarmyhammer and test again
        std::fs::create_dir_all(temp_dir.path().join(".swissarmyhammer")).unwrap();
        let dir = FileSystemIssueStorage::default_directory().unwrap();
        assert_eq!(dir, temp_dir.path().join(".swissarmyhammer/issues"));
    }
}
```

### Migration Detection Tests
```rust
#[cfg(test)]
mod migration_tests {
    use super::*;
    
    #[test]
    fn test_should_migrate_with_legacy_directory() {
        let temp_dir = TempDir::new().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();
        
        // Create legacy issues directory with content
        let issues_dir = temp_dir.path().join("issues");
        std::fs::create_dir_all(&issues_dir).unwrap();
        std::fs::write(issues_dir.join("test.md"), "test content").unwrap();
        
        assert!(FileSystemIssueStorage::should_migrate().unwrap());
    }
    
    #[test]
    fn test_should_not_migrate_when_new_exists() {
        let temp_dir = TempDir::new().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();
        
        // Create both directories
        std::fs::create_dir_all(temp_dir.path().join("issues")).unwrap();
        std::fs::create_dir_all(temp_dir.path().join(".swissarmyhammer/issues")).unwrap();
        
        assert!(!FileSystemIssueStorage::should_migrate().unwrap());
    }
    
    #[test]
    fn test_should_not_migrate_when_no_legacy() {
        let temp_dir = TempDir::new().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();
        
        assert!(!FileSystemIssueStorage::should_migrate().unwrap());
    }
    
    #[test]
    fn test_migration_info_accuracy() {
        let temp_dir = TempDir::new().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();
        
        // Create test files
        let issues_dir = temp_dir.path().join("issues");
        std::fs::create_dir_all(&issues_dir).unwrap();
        std::fs::write(issues_dir.join("file1.md"), "content1").unwrap();
        std::fs::write(issues_dir.join("file2.md"), "content2").unwrap();
        std::fs::create_dir_all(issues_dir.join("complete")).unwrap();
        std::fs::write(issues_dir.join("complete/file3.md"), "content3").unwrap();
        
        let info = FileSystemIssueStorage::migration_info().unwrap();
        
        assert!(info.should_migrate);
        assert!(info.source_exists);
        assert!(!info.destination_exists);
        assert_eq!(info.file_count, 3);
        assert!(info.total_size > 0);
    }
    
    #[test]
    fn test_migration_paths() {
        let temp_dir = TempDir::new().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();
        
        let paths = FileSystemIssueStorage::migration_paths().unwrap();
        
        assert_eq!(paths.source, temp_dir.path().join("issues"));
        assert_eq!(paths.destination, temp_dir.path().join(".swissarmyhammer/issues"));
        assert_eq!(paths.backup, temp_dir.path().join(".swissarmyhammer/issues_backup"));
    }
}
```

### Directory Counting Tests
```rust
#[cfg(test)]
mod directory_tests {
    use super::*;
    
    #[test]
    fn test_count_directory_contents_empty() {
        let temp_dir = TempDir::new().unwrap();
        let empty_dir = temp_dir.path().join("empty");
        std::fs::create_dir_all(&empty_dir).unwrap();
        
        let (count, size) = FileSystemIssueStorage::count_directory_contents(&empty_dir).unwrap();
        assert_eq!(count, 0);
        assert_eq!(size, 0);
    }
    
    #[test]
    fn test_count_directory_contents_with_files() {
        let temp_dir = TempDir::new().unwrap();
        let test_dir = temp_dir.path().join("test");
        std::fs::create_dir_all(&test_dir).unwrap();
        
        // Create test files
        std::fs::write(test_dir.join("file1.txt"), "hello").unwrap();
        std::fs::write(test_dir.join("file2.txt"), "world").unwrap();
        
        let (count, size) = FileSystemIssueStorage::count_directory_contents(&test_dir).unwrap();
        assert_eq!(count, 2);
        assert_eq!(size, 10); // "hello" + "world"
    }
    
    #[test]
    fn test_count_directory_contents_recursive() {
        let temp_dir = TempDir::new().unwrap();
        let test_dir = temp_dir.path().join("test");
        std::fs::create_dir_all(test_dir.join("subdir")).unwrap();
        
        std::fs::write(test_dir.join("root.txt"), "root").unwrap();
        std::fs::write(test_dir.join("subdir/sub.txt"), "sub").unwrap();
        
        let (count, size) = FileSystemIssueStorage::count_directory_contents(&test_dir).unwrap();
        assert_eq!(count, 2);
        assert_eq!(size, 7); // "root" + "sub"
    }
}
```

### Error Handling Tests
```rust
#[cfg(test)]
mod error_handling_tests {
    use super::*;
    
    #[test]
    fn test_default_directory_with_invalid_current_dir() {
        // This test may need special setup depending on platform
        // Test error handling when current directory is invalid
    }
    
    #[test]
    fn test_migration_info_with_permission_errors() {
        // Test graceful handling of permission denied scenarios
        let temp_dir = TempDir::new().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();
        
        let issues_dir = temp_dir.path().join("issues");
        std::fs::create_dir_all(&issues_dir).unwrap();
        
        // This test may need platform-specific permission handling
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&issues_dir).unwrap().permissions();
            perms.set_mode(0o000); // No permissions
            std::fs::set_permissions(&issues_dir, perms).unwrap();
            
            // Test that error is handled gracefully
            let result = FileSystemIssueStorage::migration_info();
            assert!(result.is_err());
            
            // Restore permissions for cleanup
            let mut perms = std::fs::metadata(&issues_dir).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&issues_dir, perms).unwrap();
        }
    }
}
```

### Integration Tests
```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[test]
    fn test_storage_creation_end_to_end() {
        let temp_dir = TempDir::new().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();
        
        // Test creation without .swissarmyhammer
        let storage1 = FileSystemIssueStorage::new_default().unwrap();
        assert!(storage1.directory_path().ends_with("issues"));
        
        // Create .swissarmyhammer and test again
        std::fs::create_dir_all(temp_dir.path().join(".swissarmyhammer")).unwrap();
        let storage2 = FileSystemIssueStorage::new_default().unwrap();
        assert!(storage2.directory_path().ends_with(".swissarmyhammer/issues"));
    }
    
    #[test]
    fn test_migration_detection_end_to_end() {
        let temp_dir = TempDir::new().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();
        
        // Initially no migration needed
        assert!(!FileSystemIssueStorage::should_migrate().unwrap());
        
        // Create legacy directory
        let issues_dir = temp_dir.path().join("issues");
        std::fs::create_dir_all(&issues_dir).unwrap();
        std::fs::write(issues_dir.join("test.md"), "content").unwrap();
        
        // Now migration should be needed
        assert!(FileSystemIssueStorage::should_migrate().unwrap());
        
        // Create new directory - migration no longer needed
        std::fs::create_dir_all(temp_dir.path().join(".swissarmyhammer/issues")).unwrap();
        assert!(!FileSystemIssueStorage::should_migrate().unwrap());
    }
}
```

## Implementation Details

### Test Organization
- Group tests by functionality (storage, migration, errors)
- Use descriptive test names that explain scenarios
- Provide helper functions for common test setup
- Ensure proper test cleanup and isolation

### Test Coverage
- Cover all new functions and methods
- Test both success and error scenarios
- Test edge cases and boundary conditions
- Test platform-specific behavior where relevant

### Performance Tests
```rust
#[test]
fn test_migration_detection_performance() {
    // Test that migration detection is fast even with large directories
    let temp_dir = TempDir::new().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();
    
    let issues_dir = temp_dir.path().join("issues");
    std::fs::create_dir_all(&issues_dir).unwrap();
    
    // Create many files
    for i in 0..1000 {
        std::fs::write(issues_dir.join(format!("file{}.md", i)), "content").unwrap();
    }
    
    let start = std::time::Instant::now();
    let should_migrate = FileSystemIssueStorage::should_migrate().unwrap();
    let duration = start.elapsed();
    
    assert!(should_migrate);
    assert!(duration < std::time::Duration::from_millis(100)); // Should be fast
}
```

## Testing Requirements

### Unit Test Coverage
- Achieve 100% line coverage for new functions
- Test all branches in conditional logic
- Test error handling paths
- Test performance characteristics

### Integration Testing
- Test interaction between components
- Test end-to-end workflows
- Test compatibility with existing code
- Test thread safety where applicable

### Cross-Platform Testing
- Test directory handling on Windows, macOS, Linux
- Test permission handling variations
- Test filesystem-specific behaviors
- Test path separator handling

## Files to Modify
- `swissarmyhammer/src/issues/filesystem.rs` (test modules)
- Add integration test files if needed
- Update test configuration for coverage
- Add performance benchmarks if needed

## Acceptance Criteria
- [ ] 100% unit test coverage for new infrastructure code
- [ ] All edge cases and error conditions tested
- [ ] Performance tests verify acceptable behavior
- [ ] Cross-platform compatibility verified
- [ ] Integration tests verify component interaction
- [ ] All tests pass consistently
- [ ] Test documentation explains complex scenarios
- [ ] CI/CD integration works properly

## Dependencies
- Depends on steps 000284-000288 being completed
- Must be completed before migration implementation starts

## Estimated Effort
~400-500 lines of comprehensive test code covering all scenarios.

## Notes
- Focus on testing the most critical functionality first
- Use property-based testing where appropriate
- Consider fuzzing for file system operations
- Document any platform-specific test limitations