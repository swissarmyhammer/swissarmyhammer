# Comprehensive Testing Suite for File Tools

Refer to /Users/wballard/github/sah-filetools/ideas/tools.md

## Objective
Create a comprehensive testing suite covering all file tools with unit tests, integration tests, security tests, and performance benchmarks.

## Testing Categories

### Unit Tests
- [ ] Individual tool functionality tests
- [ ] Parameter validation tests  
- [ ] Error condition tests
- [ ] Edge case handling tests
- [ ] Security validation tests

### Integration Tests
- [ ] MCP server integration tests
- [ ] CLI command integration tests
- [ ] Tool composition tests (using tools together)
- [ ] Workspace boundary enforcement tests
- [ ] File system interaction tests

### Security Tests
- [ ] Path traversal attack prevention tests
- [ ] Workspace boundary violation tests
- [ ] Permission escalation prevention tests
- [ ] Malformed input handling tests
- [ ] Symlink attack prevention tests

### Performance Tests
- [ ] Large file operation benchmarks
- [ ] Directory tree traversal performance
- [ ] Memory usage tests for large operations
- [ ] Concurrent operation tests
- [ ] Pattern matching performance tests

## Test Infrastructure
```rust
// Test utilities for file operations
pub struct FileTestEnvironment {
    temp_dir: TempDir,
    workspace_root: PathBuf,
}

impl FileTestEnvironment {
    pub fn new() -> Result<Self>;
    pub fn create_test_file(&self, path: &str, content: &str) -> Result<PathBuf>;
    pub fn create_test_directory(&self, path: &str) -> Result<PathBuf>;
    pub fn workspace_path(&self) -> &Path;
}
```

## Test Scenarios

### Read Tool Tests
- [ ] Basic file reading
- [ ] Offset/limit functionality
- [ ] Binary file handling
- [ ] Missing file error handling
- [ ] Permission error handling
- [ ] Large file performance

### Write Tool Tests
- [ ] New file creation
- [ ] File overwriting
- [ ] Parent directory creation
- [ ] Atomic operation verification
- [ ] Permission handling
- [ ] Disk space error handling

### Edit Tool Tests
- [ ] String replacement functionality
- [ ] Replace all vs single replacement
- [ ] Old string validation
- [ ] Atomic edit operations
- [ ] File metadata preservation
- [ ] Encoding preservation

### Glob Tool Tests
- [ ] Pattern matching accuracy
- [ ] Modification time sorting
- [ ] .gitignore integration
- [ ] Case sensitivity options
- [ ] Large directory performance
- [ ] Recursive pattern matching

### Grep Tool Tests
- [ ] Regex pattern matching
- [ ] Output mode variations
- [ ] File type filtering
- [ ] Context line extraction
- [ ] Case sensitivity options
- [ ] Performance with large files

## Property-Based Testing
- [ ] Random file path generation and validation
- [ ] Random content generation for edit operations
- [ ] Random glob pattern generation
- [ ] Random regex pattern testing
- [ ] Fuzz testing for security vulnerabilities

## Test Data and Fixtures
- [ ] Sample files for testing (text, binary, various encodings)
- [ ] Test directory structures
- [ ] .gitignore test cases
- [ ] Large file generation utilities
- [ ] Malformed input test cases

## Acceptance Criteria
- [ ] 95%+ code coverage across all file tools
- [ ] All security scenarios tested and validated
- [ ] Performance benchmarks established
- [ ] Integration tests pass with MCP server
- [ ] CLI integration tests complete
- [ ] Property-based tests identify no issues
- [ ] All edge cases covered with appropriate error handling