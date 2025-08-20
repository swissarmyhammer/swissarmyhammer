# Comprehensive Integration Testing for Directory Migration

Refer to /Users/wballard/github/sah-directory/ideas/directory.md

## Overview
Create comprehensive integration tests to validate the complete directory migration system, including real-world scenarios, error cases, and cross-component interactions.

## Test Coverage Strategy

### End-to-End Workflow Tests
```rust
#[tokio::test]
async fn test_complete_workflow_in_git_repository() {
    let temp_git_repo = create_test_git_repository();
    
    // Test complete workflow: memo -> todo -> search -> workflow execution
    // Verify all components use the same .swissarmyhammer directory
    // Validate data persistence and consistency
}

#[tokio::test]  
async fn test_workflow_outside_git_repository() {
    let temp_dir = create_non_git_directory();
    
    // Test that components fail gracefully with clear error messages
    // Verify no partial data creation
    // Validate error message consistency
}
```

### Component Integration Tests
1. **File Loader + Other Systems**: Validate prompt loading works with new directory structure
2. **Search + Memo Integration**: Verify search indexing works with Git-centric memo storage
3. **Todo + Workflow Integration**: Validate todo system works with workflow execution
4. **Doctor + All Components**: Verify doctor command validates entire system health

### Migration Scenario Tests  
```rust
#[tokio::test]
async fn test_migration_from_multiple_directories() {
    // Create old-style multiple .swissarmyhammer directories
    let multi_dir_setup = create_legacy_directory_structure();
    
    // Run migration validation
    let scan_result = scan_existing_directories();
    assert!(scan_result.conflicts.is_empty());
    
    // Verify migration plan is safe
    let migration_plan = validate_migration_safety(&git_root)?;
    assert!(migration_plan.is_safe);
}
```

### Real Repository Tests
```rust
#[tokio::test]
async fn test_with_actual_git_repository() {
    // Create real Git repository with proper .git structure
    let git_repo = Repository::init(&temp_dir)?;
    
    // Test all operations work with real Git repository
    // Verify .swissarmyhammer directory creation at correct location  
    // Validate all components can read/write successfully
}
```

## Error Scenario Testing

### Git Repository Edge Cases
- Bare repositories
- Corrupt `.git` directories  
- Permission-denied scenarios
- Very deep directory hierarchies
- Repositories within repositories (nested Git)

### File System Edge Cases
- Read-only filesystems
- Network mounted directories
- Case-sensitive vs case-insensitive filesystems
- Long path names
- Special characters in directory names

### Cross-Platform Testing
- Windows path handling (drive letters, backslashes)
- macOS case-insensitive filesystem behavior
- Linux permission and ownership scenarios
- Network filesystem behaviors

## Performance Testing

### Large Repository Tests
```rust
#[tokio::test] 
async fn test_performance_with_large_repository() {
    let large_repo = create_large_git_repository(1000); // 1000 commits
    
    // Measure directory resolution performance
    let start = Instant::now();
    let swissarmyhammer_dir = find_swissarmyhammer_directory();
    let duration = start.elapsed();
    
    assert!(duration < Duration::from_millis(100)); // Should be fast
}
```

### Concurrent Access Tests
```rust
#[tokio::test]
async fn test_concurrent_component_access() {
    let git_repo = create_test_git_repository();
    
    // Simulate concurrent access from multiple components
    let futures = vec![
        test_memo_operations(),
        test_todo_operations(), 
        test_search_operations(),
    ];
    
    try_join_all(futures).await?;
    // Verify no race conditions or data corruption
}
```

## Compatibility Testing

### Backwards Compatibility Validation
- Verify old commands fail with helpful messages
- Test migration paths from old directory structures
- Validate data preservation during migration

### Forward Compatibility Planning
- Test directory structure extensibility
- Verify new subdirectory creation works
- Validate component isolation

## Test Infrastructure

### Test Utilities  
```rust
pub struct GitRepositoryTestGuard {
    temp_dir: TempDir,
    git_repo: Repository,
}

impl GitRepositoryTestGuard {
    pub fn new() -> Self {
        // Create isolated Git repository for testing
    }
    
    pub fn swissarmyhammer_dir(&self) -> PathBuf {
        self.temp_dir.path().join(".swissarmyhammer")
    }
}
```

### Isolated Test Environments
- Each test gets its own temporary Git repository
- Environment variables properly isolated
- No test interference or shared state

## Tasks
1. Create comprehensive test utilities for Git repository testing
2. Implement end-to-end workflow integration tests  
3. Add migration scenario testing with real directory structures
4. Create error case testing for all failure modes
5. Add performance testing for directory resolution
6. Implement cross-platform compatibility tests
7. Add concurrent access testing
8. Create test documentation and examples

## Test Metrics
- **Coverage**: 95%+ line coverage for directory-related code
- **Performance**: Directory resolution < 100ms in worst case
- **Reliability**: 0 flaky tests, all tests pass consistently
- **Compatibility**: Tests pass on Windows, macOS, Linux

## Dependencies
- Depends on: All previous migration steps (000001-000009)

## Success Criteria
- Comprehensive test coverage validates entire migration system
- All edge cases and error scenarios properly tested
- Performance meets requirements across all platforms
- Tests provide confidence for production deployment
- Test suite serves as documentation for expected behavior
- Migration scenarios thoroughly validated with real data