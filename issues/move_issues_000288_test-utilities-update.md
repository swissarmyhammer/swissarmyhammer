# Update Test Utilities for New Directory Structure

## Overview
Update test utilities in `swissarmyhammer-tools/src/test_utils.rs` and related test infrastructure to use the new `.swissarmyhammer/issues` directory structure.

Refer to /Users/wballard/github/sah-issues/ideas/move_issues.md

## Current State
- **File**: `swissarmyhammer-tools/src/test_utils.rs:19`
- **Current Logic**: `PathBuf::from("./test_issues")`
- **Issue**: Test isolation uses legacy directory pattern

## Target Implementation

### Update Test Context Creation
```rust
pub async fn create_test_context() -> ToolContext {
    let issue_storage: Arc<RwLock<Box<dyn IssueStorage>>> = Arc::new(RwLock::new(Box::new(
        FileSystemIssueStorage::new(PathBuf::from("./.swissarmyhammer/test_issues")).unwrap(),
    )));
    // ... rest unchanged
}
```

### Enhanced Test Directory Structure
```rust
/// Create isolated test environment with proper directory structure
pub fn create_isolated_test_environment() -> IsolatedTestEnvironment {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp directory");
    let swissarmyhammer_dir = temp_dir.path().join(".swissarmyhammer");
    
    // Create the full .swissarmyhammer structure
    std::fs::create_dir_all(&swissarmyhammer_dir).expect("Failed to create .swissarmyhammer");
    std::fs::create_dir_all(swissarmyhammer_dir.join("issues")).expect("Failed to create issues dir");
    std::fs::create_dir_all(swissarmyhammer_dir.join("issues/complete")).expect("Failed to create complete dir");
    
    IsolatedTestEnvironment::new(temp_dir)
}
```

### Test Environment Helpers
```rust
pub struct TestIssueEnvironment {
    pub temp_dir: TempDir,
    pub issues_dir: PathBuf,
    pub complete_dir: PathBuf,
}

impl TestIssueEnvironment {
    pub fn new() -> Self {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp directory");
        let swissarmyhammer_dir = temp_dir.path().join(".swissarmyhammer");
        let issues_dir = swissarmyhammer_dir.join("issues");
        let complete_dir = issues_dir.join("complete");
        
        // Create directory structure
        std::fs::create_dir_all(&complete_dir).expect("Failed to create directory structure");
        
        Self {
            temp_dir,
            issues_dir,
            complete_dir,
        }
    }
    
    pub fn storage(&self) -> FileSystemIssueStorage {
        FileSystemIssueStorage::new(self.issues_dir.clone()).unwrap()
    }
    
    pub fn path(&self) -> &Path {
        self.temp_dir.path()
    }
}
```

### Update Existing Test Helpers
```rust
/// Update IsolatedTestEnvironment to include .swissarmyhammer structure
impl IsolatedTestEnvironment {
    pub fn new() -> Self {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp directory");
        let swissarmyhammer_dir = temp_dir.path().join(".swissarmyhammer");
        
        // Create standard .swissarmyhammer structure
        std::fs::create_dir_all(&swissarmyhammer_dir).expect("Failed to create .swissarmyhammer");
        std::fs::create_dir_all(swissarmyhammer_dir.join("issues")).expect("Failed to create issues");
        std::fs::create_dir_all(swissarmyhammer_dir.join("issues/complete")).expect("Failed to create complete");
        
        // Set up environment variables
        std::env::set_var("HOME", temp_dir.path());
        std::env::set_var("PWD", temp_dir.path());
        
        Self { temp_dir }
    }
    
    pub fn issues_dir(&self) -> PathBuf {
        self.temp_dir.path().join(".swissarmyhammer/issues")
    }
    
    pub fn complete_dir(&self) -> PathBuf {
        self.issues_dir().join("complete")
    }
}
```

## Implementation Details

### Test Directory Structure
Create consistent test directory structure across all test environments:
```
temp_test_dir/
├── .swissarmyhammer/
│   ├── issues/
│   │   └── complete/
│   └── (other SAH directories)
```

### Test Isolation Improvements
- Ensure each test gets isolated `.swissarmyhammer` structure
- Provide convenient helper methods for test setup
- Maintain compatibility with existing test patterns
- Add helper methods for common test operations

### Integration with Existing Patterns
- Update `IsolatedTestEnvironment` to create proper structure
- Maintain RAII cleanup patterns
- Preserve thread safety for parallel tests
- Keep existing test utility interfaces where possible

### Performance Considerations
- Minimize directory creation overhead
- Reuse directory templates where possible
- Avoid unnecessary filesystem operations in tests
- Optimize cleanup for large test suites

## Testing Requirements

### Test Infrastructure Tests
- Test that `IsolatedTestEnvironment` creates correct structure
- Test directory cleanup works properly
- Test parallel test isolation
- Test helper method functionality

### Integration with Existing Tests
- Update all existing tests to use new test utilities
- Verify no test regressions from directory changes
- Test backward compatibility scenarios
- Test migration scenarios in test environments

### Cross-Platform Testing
- Test directory creation on different platforms
- Test permission handling in test environments
- Test path handling with various filesystems
- Test cleanup behavior on different platforms

## Files to Modify
- `swissarmyhammer-tools/src/test_utils.rs`
- All test files using the test utilities
- Integration test configurations
- Test documentation and examples

## Migration of Existing Tests

### Pattern Updates
Update test patterns from:
```rust
// Old pattern
let storage = FileSystemIssueStorage::new("./test_issues")?;

// New pattern  
let test_env = TestIssueEnvironment::new();
let storage = test_env.storage();
```

### Test File Updates
- Review all test files for hardcoded issue paths
- Update test assertions to use new directory structure
- Update test data creation to use new helpers
- Verify test cleanup still works correctly

## Acceptance Criteria
- [ ] Test utilities create proper `.swissarmyhammer` structure
- [ ] All existing tests pass with updated utilities
- [ ] Test isolation maintained for parallel execution
- [ ] New helper methods simplify test setup
- [ ] Backward compatibility maintained where possible
- [ ] Performance regression avoided in test execution
- [ ] Cross-platform compatibility maintained
- [ ] RAII cleanup patterns preserved

## Dependencies
- Depends on steps 000284-000287 for core infrastructure
- Should be done before migration implementation (steps 000290+)

## Estimated Effort
~250-300 lines of code changes including test updates and helper methods.

## Notes
- Focus on maintaining existing test patterns while supporting new structure
- Consider creating migration-specific test helpers
- Ensure test utilities support both legacy and new directory structures for migration testing