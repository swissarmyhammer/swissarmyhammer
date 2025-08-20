# Directory Integration Tests

Comprehensive integration test suite for SwissArmyHammer's Git repository-centric directory system migration.

## Overview

This test suite validates the complete directory migration system that transitions SwissArmyHammer from legacy directory structures to a Git repository-centric approach where `.swissarmyhammer` directories exist only at Git repository roots.

## Test Structure

```
tests/directory_integration/
├── mod.rs                    # Core test utilities and infrastructure
├── end_to_end_tests.rs      # Complete workflow integration tests
├── migration_tests.rs       # Legacy directory migration scenarios
├── error_scenario_tests.rs  # Edge cases and error handling
├── performance_tests.rs     # Performance validation and benchmarks
├── concurrent_tests.rs      # Thread safety and concurrent access
└── README.md               # This documentation
```

## Test Categories

### End-to-End Workflow Tests (`end_to_end_tests.rs`)

Tests complete workflows that span multiple SwissArmyHammer components:

- **Memo Lifecycle Testing**: Create, read, and manage memos in Git-centric structure
- **Todo Workflow Integration**: Todo list operations across repository subdirectories  
- **Search System Integration**: Semantic search with Git-centric database location
- **Issues System Integration**: Issue tracking with proper directory resolution
- **Multi-Component Workflows**: Complex scenarios using multiple systems together

**Key Features:**
- Tests from various subdirectories to ensure consistent directory resolution
- Validates data persistence across component boundaries
- Performance requirements: All operations < 500ms timeout
- Cross-component data consistency validation

### Migration Scenario Tests (`migration_tests.rs`)

Tests migration from legacy directory structures to Git-centric approach:

- **Single Directory Migration**: Migration from existing `.swissarmyhammer` directory
- **Multiple Directory Migration**: Handling multiple `.swissarmyhammer` directories in hierarchy
- **Nested Git Repository Scenarios**: Migration in repositories with nested Git repos
- **Data Preservation Testing**: Ensuring all existing data is preserved during migration
- **Git Worktree Support**: Migration in Git worktree environments
- **Deep Directory Structures**: Migration with deeply nested directory hierarchies

**Key Features:**
- Comprehensive data preservation validation
- Performance testing with large directory structures
- Conflict resolution and error handling
- Rollback scenario testing

### Error Scenario Tests (`error_scenario_tests.rs`)

Tests error handling and edge cases:

- **Non-Git Repository Scenarios**: Proper error handling outside Git repositories
- **Corrupt Git Repository Handling**: Graceful degradation with corrupt `.git` directories
- **File System Edge Cases**: Read-only filesystems, permission issues, special characters
- **Symbolic Link Handling**: Proper resolution through symbolic links
- **Path Length Limits**: Very long paths and filename handling
- **Malformed Directory Structures**: Recovery from corrupted `.swissarmyhammer` directories

**Key Features:**
- Comprehensive error message validation
- Security boundary testing
- Cross-platform compatibility (Windows, macOS, Linux)
- Graceful degradation under adverse conditions

### Performance Tests (`performance_tests.rs`)

Tests performance characteristics and establishes benchmarks:

- **Basic Directory Resolution**: Standard performance benchmarks
- **Deep Directory Structures**: Performance with deeply nested directories
- **Large Repository Testing**: Performance in repositories with many files/commits
- **High-Frequency Operations**: Burst operation performance
- **Concurrent Performance**: Performance under concurrent load
- **Regression Testing**: Ensuring no performance degradation

**Key Performance Targets:**
- Git repository detection: < 20ms
- SwissArmyHammer directory detection: < 25ms  
- Directory creation: < 100ms
- 1000 operations: < 1.5 seconds
- No operation should be >3x average time

### Concurrent Tests (`concurrent_tests.rs`)

Tests thread safety and concurrent access patterns:

- **Concurrent Directory Resolution**: Multiple threads resolving directories
- **Concurrent Directory Creation**: Race condition handling during creation
- **Concurrent File Operations**: Thread-safe file operations within `.swissarmyhammer`
- **Cross-Subdirectory Concurrency**: Threads working from different locations
- **Rapid Directory Changes**: Thread safety with rapid directory changes
- **Stress Testing**: High-load concurrent scenarios

**Key Features:**
- Thread safety validation
- Race condition prevention
- Data consistency under concurrent access
- Performance maintenance under load

## Test Infrastructure

### GitRepositoryTestGuard

Core test utility providing isolated Git repository environments:

```rust
// Basic usage
let guard = GitRepositoryTestGuard::new();

// With .swissarmyhammer directory
let guard = GitRepositoryTestGuard::new_with_swissarmyhammer();

// With realistic project structure
let guard = GitRepositoryTestGuard::new_with_swissarmyhammer()
    .with_project_structure();

// Advanced scenarios
let guard = GitRepositoryTestGuard::new()
    .with_swissarmyhammer()
    .with_project_structure()
    .as_git_worktree();
```

**Features:**
- Isolated temporary Git repositories
- Automatic cleanup via RAII
- Parallel test execution support
- Working directory restoration
- Git worktree simulation
- Deep directory structure creation

### Performance Utilities

```rust
// Measure operation time
let (result, duration) = measure_time(|| {
    find_swissarmyhammer_directory()
});

// Create large repository for performance testing
let guard = create_large_git_repository(commits: 10, files_per_commit: 20);

// Generate unique test identifiers
let test_id = generate_test_id();
```

### Legacy Structure Simulation

```rust
// Create legacy directory structure for migration testing
let (temp_dir, deepest_path, swissarmyhammer_dirs) = create_legacy_directory_structure();
```

## Running Tests

### Basic Test Execution

```bash
# Run all directory integration tests
cargo test directory_integration

# Run specific test modules
cargo test directory_integration::end_to_end
cargo test directory_integration::migration
cargo test directory_integration::error_scenario
cargo test directory_integration::performance
cargo test directory_integration::concurrent

# Run with output for debugging
cargo test directory_integration -- --nocapture

# Run specific test function
cargo test test_complete_memo_workflow_in_git_repository
```

### Performance Testing

```bash
# Run only performance tests
cargo test directory_integration::performance

# Run performance tests with timing output
cargo test directory_integration::performance -- --nocapture

# Run single-threaded for accurate performance measurement
cargo test directory_integration::performance -- --test-threads=1
```

### Concurrent Testing

```bash
# Run concurrent tests (these require multiple threads)
cargo test directory_integration::concurrent

# Stress test with maximum parallelism
cargo test directory_integration::concurrent -- --test-threads=8
```

### Platform-Specific Testing

```bash
# Windows-specific features (run on Windows)
cargo test directory_integration::error_scenario -- --nocapture

# Unix-specific features (run on Linux/macOS)
cargo test test_symbolic_links

# Cross-platform compatibility
cargo test test_special_characters_in_paths
```

## Test Coverage

The integration test suite provides comprehensive coverage:

- **Line Coverage**: 95%+ for directory-related code
- **Scenario Coverage**: All major use cases and edge cases
- **Platform Coverage**: Windows, macOS, Linux compatibility
- **Performance Coverage**: Benchmarks for all critical operations
- **Concurrency Coverage**: Thread safety validation

### Coverage Analysis

```bash
# Install coverage tools
cargo install cargo-tarpaulin

# Run coverage analysis
cargo tarpaulin --out Html --output-dir coverage -- directory_integration

# View coverage report
open coverage/tarpaulin-report.html
```

## Debugging Tests

### Common Issues

1. **Test Timeouts**: Some performance tests have strict timing requirements
   ```bash
   # Run with relaxed timing
   RUST_TEST_TIME_UNIT=10000 cargo test directory_integration::performance
   ```

2. **File System Permissions**: Some tests modify permissions
   ```bash
   # Ensure proper cleanup
   find /tmp -name "*swissarmyhammer*" -exec rm -rf {} \; 2>/dev/null
   ```

3. **Parallel Test Conflicts**: Tests are designed for parallel execution
   ```bash
   # Force sequential execution if needed
   cargo test directory_integration -- --test-threads=1
   ```

### Debug Output

```bash
# Enable debug logging
RUST_LOG=debug cargo test directory_integration -- --nocapture

# Show test output
cargo test directory_integration -- --nocapture

# Run specific failing test
cargo test test_concurrent_directory_creation -- --nocapture
```

### Test Environment Variables

```bash
# Skip performance-sensitive tests in CI
SKIP_PERFORMANCE_TESTS=1 cargo test directory_integration

# Increase timeout for slow systems
TEST_TIMEOUT_MS=5000 cargo test directory_integration::performance

# Enable verbose output
VERBOSE_TESTS=1 cargo test directory_integration
```

## Continuous Integration

The tests are designed to run reliably in CI environments:

### GitHub Actions Configuration

```yaml
- name: Run Directory Integration Tests
  run: |
    cargo test directory_integration
    cargo test directory_integration::performance -- --test-threads=1
    cargo test directory_integration::concurrent
  env:
    RUST_LOG: info
    TEST_TIMEOUT_MS: 10000
```

### Performance Monitoring

The tests establish performance baselines and will fail if operations exceed acceptable thresholds:

- Directory resolution operations must complete within defined time limits
- Memory usage is monitored to prevent leaks
- Concurrent operations must maintain performance under load

## Extending the Tests

### Adding New Test Cases

1. **End-to-End Tests**: Add to `end_to_end_tests.rs`
   ```rust
   #[tokio::test]
   async fn test_new_workflow_integration() {
       let guard = GitRepositoryTestGuard::new_with_swissarmyhammer();
       // Test implementation
   }
   ```

2. **Migration Tests**: Add to `migration_tests.rs`
   ```rust
   #[test]
   fn test_new_migration_scenario() {
       let guard = GitRepositoryTestGuard::new();
       // Migration test implementation
   }
   ```

3. **Performance Tests**: Add to `performance_tests.rs`
   ```rust
   #[test]
   fn test_new_performance_scenario() {
       let guard = GitRepositoryTestGuard::new_with_swissarmyhammer();
       let (result, duration) = measure_time(|| {
           // Operation to measure
       });
       assert!(duration < Duration::from_millis(expected_ms));
   }
   ```

### Test Utilities

When adding new tests, leverage existing utilities:

- Use `GitRepositoryTestGuard` for isolated environments
- Use `measure_time()` for performance validation
- Use `generate_test_id()` for unique identifiers
- Follow existing naming patterns for consistency

## Quality Assurance

This test suite serves as the quality gate for the directory migration system:

- **Regression Prevention**: Comprehensive coverage prevents regressions
- **Performance Assurance**: Benchmarks ensure acceptable performance
- **Reliability Testing**: Concurrent and stress testing validates reliability
- **Cross-Platform Compatibility**: Platform-specific testing ensures broad compatibility

The tests must pass on all supported platforms before any directory-related changes can be merged.