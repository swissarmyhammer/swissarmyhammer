# Step 4: Optimize File System Heavy Tests

Refer to /Users/wballard/sah-slow_tests/ideas/slow_tests.md

## Objective
Optimize tests with extensive file I/O operations, directory traversals, and file processing to reduce execution time while maintaining test coverage and reliability.

## Background
The SwissArmyHammer codebase has extensive file system testing including:
- Issue storage and migration tests (`swissarmyhammer/src/issues/filesystem.rs`)
- File tool integration tests (`swissarmyhammer-tools/tests/file_tools_*`)  
- Search indexing tests with large file operations
- Git repository manipulation tests
- Configuration file parsing and validation tests

## Tasks

### 1. Identify File System Test Bottlenecks
- Audit tests involving heavy file I/O operations
- Identify tests creating large directory structures
- Document tests with expensive file processing operations
- Map tests using real vs. in-memory file operations

### 2. Optimize File Operations
- **In-Memory File Systems**: Use `tempfile` and in-memory alternatives where appropriate
- **Minimal Test Data**: Replace large test files with minimal data that validates functionality  
- **Lazy File Creation**: Create test files only when needed, not in setup
- **Batch Operations**: Group file operations to reduce syscall overhead
- **Concurrent File Access**: Ensure file tests can run in parallel with unique paths

### 3. Optimize Directory Operations  
- **Reduce Directory Depth**: Use shallow directory structures for tests
- **Limit Directory Size**: Use minimal number of files needed for validation
- **Avoid Recursive Operations**: Replace deep traversals with focused tests
- **Efficient Cleanup**: Use RAII patterns for automatic cleanup

### 4. Split Large File Processing Tests
Break down tests that process large datasets:
- **Unit Tests**: Test file processing logic with minimal data
- **Component Tests**: Test file operations with mock file systems  
- **Integration Tests**: Test complete workflows with optimized test data
- **Performance Tests**: Separate true performance tests from functional tests

### 5. Implement File System Test Optimizations

#### In-Memory File System Pattern
```rust
use tempfile::{TempDir, NamedTempFile};

#[test]
fn test_file_operations() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.txt");
    
    // File operations on isolated temporary path
    std::fs::write(&test_file, "test content").unwrap();
    
    // Test validations
    assert_eq!(std::fs::read_to_string(&test_file).unwrap(), "test content");
    // TempDir automatically cleaned up
}
```

#### Minimal Test Data Pattern  
```rust
// Instead of large test files
const LARGE_TEST_FILE: &str = include_str!("../fixtures/large_file.txt");

// Use minimal data that validates functionality
const MINIMAL_TEST_DATA: &str = "line1\nline2\n";
```

## Acceptance Criteria
- [ ] All file system heavy tests identified and documented
- [ ] Tests optimized to use minimal necessary file operations
- [ ] In-memory alternatives implemented where appropriate
- [ ] Large file processing tests split into focused components
- [ ] Tests use unique temporary directories for parallel execution
- [ ] File system test execution time reduced by >40%  
- [ ] All file system test coverage maintained
- [ ] No shared file dependencies between tests

## Implementation Strategy

### Test Categories to Optimize
1. **Issue Storage Tests** - Migration and filesystem storage operations
2. **File Tool Tests** - File manipulation and processing operations
3. **Search Index Tests** - File indexing and processing operations  
4. **Configuration Tests** - Config file parsing and validation
5. **Git Integration Tests** - Repository file operations

### Specific Optimizations
- Replace large test fixtures with minimal data
- Use `std::fs::write` instead of complex file creation
- Implement lazy loading for test resources
- Add parallel file operation support
- Use memory-mapped files for read-heavy operations

## Estimated Effort  
Large (5-6 focused work sessions)

## Dependencies
- Step 2 (serial test fixes for parallel file operations)

## Follow-up Steps
- Step 5: Optimize Database and Search Tests
- File system optimizations will improve many test categories