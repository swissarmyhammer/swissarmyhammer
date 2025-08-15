# Implement Comprehensive Memoranda Testing

## Overview
Create a complete test suite for all memoranda functionality, ensuring reliability and correctness across storage, MCP, CLI, and search operations.

## Tasks

### 1. Unit Tests
- **Storage Tests** (`swissarmyhammer/src/memoranda/storage.rs`):
  - CRUD operations with various data sizes
  - Directory creation and permission handling
  - File corruption recovery
  - Error condition handling
  - Concurrent access testing
- **Data Structure Tests** (extend existing in `mod.rs`):
  - Serialization edge cases
  - Invalid data handling
  - ULID generation and validation

### 2. Integration Tests  
- **MCP Integration Tests** (`swissarmyhammer/tests/mcp_memoranda_tests.rs`):
  - End-to-end MCP tool testing
  - Error response formatting
  - Large memo handling
  - Concurrent MCP requests
- **CLI Integration Tests** (`swissarmyhammer-cli/tests/memo_cli_tests.rs`):
  - All CLI commands working correctly
  - Stdin/stdout handling
  - Error exit codes
  - Command completion

### 3. Performance Tests
- **Storage Performance** (`swissarmyhammer/benches/memo_benchmarks.rs`):
  - Benchmark memo creation/retrieval with large collections
  - Search performance with various query types
  - Memory usage patterns
- **MCP Performance Tests**:
  - Response time testing for all MCP operations
  - Large context retrieval performance

### 4. Edge Case Testing
- **File System Edge Cases**:
  - No write permissions
  - Disk full scenarios  
  - Corrupted memo files
  - Very large memo content (approaching 1MB limit)
- **Search Edge Cases**:
  - Empty search queries
  - Special characters in search terms
  - Very long search queries
  - Unicode content search

### 5. Mock and Test Utilities
- Create mock storage implementation for testing
- Test data generation utilities
- Helper functions for integration test setup
- Temporary directory management for tests

### 6. Property-Based Testing
- Use `proptest` crate for property-based testing:
  - Round-trip serialization properties
  - Search result consistency
  - ULID uniqueness properties
  - Storage operation invariants

## Test Coverage Goals
- **Unit tests**: >95% code coverage
- **Integration tests**: All user-facing operations
- **Performance tests**: Baseline performance metrics established
- **Edge cases**: All known failure modes tested

## Test Categories
```rust
// Example test structure
mod storage_tests {
    mod unit_tests { /* ... */ }
    mod integration_tests { /* ... */ }
    mod performance_tests { /* ... */ }
    mod edge_case_tests { /* ... */ }
}

mod mcp_tests { /* ... */ }
mod cli_tests { /* ... */ }  
mod search_tests { /* ... */ }
```

## Implementation Notes
- Follow existing test patterns from issues and other modules
- Use appropriate test fixtures and helper utilities
- Ensure tests are deterministic and can run in parallel
- Include both positive and negative test cases

## Proposed Solution

Based on analyzing the existing codebase, I will implement comprehensive testing following these patterns:

### 1. Unit Tests in memoranda/storage.rs
- Test FileSystemMemoStorage CRUD operations with real filesystem
- Concurrent access tests using serial_test crate for isolation
- Error condition testing (corrupted files, permission errors)
- Directory creation and cleanup tests
- File locking and atomic operations testing

### 2. Extended Data Structure Tests in memoranda/mod.rs
- Property-based tests with proptest for ULID generation/validation
- Serialization round-trip tests for all request/response types
- Edge case validation (empty strings, unicode content, very large content)
- SearchOptions and ContextOptions validation

### 3. MCP Integration Tests (swissarmyhammer/tests/mcp_memoranda_tests.rs)
- Following existing MCP test patterns from mcp_integration_test.rs
- Test all MCP tool handlers for memo operations
- Error response formatting and validation
- Large memo content handling
- Concurrent MCP request handling

### 4. CLI Integration Tests (swissarmyhammer-cli/tests/memo_cli_tests.rs)
- Following existing CLI test patterns from cli_integration_test.rs
- Test all memo CLI subcommands (create, get, list, search, delete, context)
- Stdin/stdout handling with different input formats
- Error exit codes validation
- Command completion and help text

### 5. Performance Benchmarks (swissarmyhammer/benches/memo_benchmarks.rs)
- Using criterion crate following existing benchmark patterns
- Benchmark memo creation/retrieval with varying collection sizes
- Search performance tests with different query types and result counts
- Memory usage profiling for large memo collections
- Advanced search engine performance testing

### 6. Edge Case Testing
- File system permission scenarios using tempfile for isolation
- Disk space exhaustion simulation
- Corrupted JSON file recovery testing  
- Unicode content edge cases
- Search query boundary conditions
- Very large memo content (approaching theoretical limits)

### 7. Mock Storage Implementation  
- Create MockMemoStorage for deterministic testing
- Implement all MemoStorage trait methods with configurable behaviors
- Support error injection for failure scenario testing
- Memory-based storage for fast test execution

### 8. Property-Based Testing with Proptest
- ULID uniqueness and ordering properties
- Serialization round-trip properties for all data structures
- Search result consistency across different query formulations
- Storage operation invariants (create->get->delete cycles)

### Implementation Approach:
1. Create comprehensive unit tests first (TDD approach)
2. Build integration test infrastructure  
3. Add performance benchmarks with baseline metrics
4. Implement edge case and property-based tests
5. Add mock utilities for deterministic testing
6. Verify >95% test coverage using cargo tarpaulin

## Acceptance Criteria
- [ ] All core functionality covered by unit tests
- [ ] MCP integration tests passing
- [ ] CLI integration tests passing  
- [ ] Performance benchmarks established
- [ ] Edge case tests covering known failure modes
- [ ] All tests passing in CI/CD pipeline
- [ ] Test coverage reports showing >90% coverage