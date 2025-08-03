# Add Comprehensive Testing for Todo Tools

Refer to ./specification/todo_tool.md

## Overview
Implement thorough test coverage for all todo tool functionality including unit tests, integration tests, and property-based testing.

## Testing Strategy
Following the established testing patterns in the codebase:
- Unit tests for individual tool components
- Integration tests for tool workflows
- Property-based testing for data validation
- Error condition testing
- Concurrent access testing

## Test Categories

### Unit Tests
For each tool (`create`, `show`, `mark_complete`):
- Tool creation and basic properties
- Schema validation
- Parameter parsing
- Error handling for invalid inputs
- Tool registration verification

### Integration Tests
- End-to-end workflow testing
- File system integration
- YAML serialization round-trips
- Multi-tool coordination (create -> show -> mark_complete)
- Gitignore management verification

### Property-Based Testing
Using `proptest` crate for:
- ULID generation and ordering properties
- YAML serialization invariants
- File name validation edge cases
- Concurrent operation safety

### Error Condition Testing
- File not found scenarios
- Permission denied errors
- Invalid ULID formats
- Malformed YAML files
- Disk space exhaustion simulation
- Concurrent access conflicts

### Performance Testing
- Large todo list handling
- Concurrent operation performance
- File I/O optimization validation
- Memory usage profiling

## Test Structure
```
tests/
├── todo_integration_tests.rs
├── todo_unit_tests.rs
├── todo_property_tests.rs
└── todo_concurrent_tests.rs

src/mcp/tools/todo/
├── create/
│   └── mod.rs (with #[cfg(test)] module)
├── show/
│   └── mod.rs (with #[cfg(test)] module)
└── mark_complete/
    └── mod.rs (with #[cfg(test)] module)
```

## Mock and Test Utilities
- Mock file system for isolated testing
- Test data generation utilities
- Cleanup utilities for test artifacts
- Concurrent test orchestration helpers

## Testing Scenarios
1. **Session Workflow**: Create list -> Add items -> Work through items -> Complete session
2. **Multi-List Management**: Multiple concurrent todo lists
3. **Error Recovery**: Handling corrupted files and recovery
4. **FIFO Ordering**: Verify "next" item selection works correctly
5. **Persistence**: Data survives tool restarts
6. **Security**: Path traversal and injection attempts

## Success Criteria
- 100% line coverage for todo tool code
- All edge cases and error conditions covered
- Property-based tests validate invariants
- Integration tests cover complete workflows
- Performance tests validate acceptable limits
- Concurrent tests ensure thread safety
- Security tests prevent malicious usage

## Implementation Notes
- Use existing test utilities from the codebase
- Follow established testing patterns
- Include both positive and negative test cases
- Test with realistic data sizes and scenarios
- Validate error messages are helpful
- Ensure tests are deterministic and fast
- Use `serial_test` for tests requiring isolation