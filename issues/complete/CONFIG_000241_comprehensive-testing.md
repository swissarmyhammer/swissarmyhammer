# CONFIG_000241: Comprehensive Testing - sah.toml Configuration

Refer to ./specification/config.md

## Goal

Implement comprehensive test coverage for the sah.toml configuration system, including unit tests, integration tests, and edge case validation.

## Tasks

1. **Unit Tests for Core Components**
   - Test ConfigValue enum conversion and serialization
   - Test Configuration struct with nested tables and dot notation
   - Test TOML parsing with various valid and invalid inputs
   - Test environment variable substitution with different scenarios
   - Test error handling and error message quality

2. **Integration Tests**
   - Test end-to-end configuration loading from file system
   - Test template integration with configuration variables
   - Test workflow execution with configuration variables available
   - Test CLI commands for configuration management
   - Test file discovery from different directory structures

3. **Security and Validation Tests**
   - Test file size limits (1MB maximum)
   - Test depth limits (10 levels maximum)
   - Test path traversal prevention
   - Test malformed TOML handling
   - Test invalid environment variable syntax

4. **Error Scenario Testing**
   - Test missing configuration files
   - Test unreadable files (permission issues)
   - Test corrupted TOML files
   - Test circular references in included files
   - Test invalid variable names and values

5. **Performance and Caching Tests**
   - Test configuration caching behavior
   - Test file modification time tracking
   - Test performance with large configuration files
   - Test memory usage with complex nested structures

6. **Template Integration Tests**
   - Test configuration variables in liquid templates
   - Test variable precedence (config vs workflow vs explicit)
   - Test nested object access via dot notation
   - Test configuration variables in all action types

## Acceptance Criteria

- [ ] Unit test coverage >90% for all configuration modules
- [ ] Integration tests cover end-to-end configuration workflows
- [ ] Security tests prevent all identified attack vectors
- [ ] Error scenarios properly tested with expected outcomes
- [ ] Performance tests validate caching and memory usage
- [ ] Template integration fully tested across all action types
- [ ] Property-based tests for complex scenarios where applicable

## Files to Create

- `swissarmyhammer/src/config/tests/unit_tests.rs` - Unit tests
- `swissarmyhammer/src/config/tests/integration_tests.rs` - Integration tests
- `swissarmyhammer/src/config/tests/security_tests.rs` - Security validation tests
- `swissarmyhammer-cli/tests/config_cli_tests.rs` - CLI integration tests

## Files to Modify

- `swissarmyhammer/src/config/mod.rs` - Add test modules

## Test Data Requirements

- Sample sah.toml files with various complexity levels
- Invalid TOML files for error testing
- Large configuration files for performance testing
- Environment variable test scenarios

## Next Steps

After completion, proceed to LOG_000242_liquid-rendering-fix for addressing the current issue with Log action template rendering.
## Proposed Solution

After analyzing the existing sah.toml configuration system, I will implement comprehensive test coverage following the TDD approach and existing testing patterns. The solution will be structured in multiple test modules:

### Test Architecture Overview

1. **Unit Tests** - Test individual components in isolation
   - `swissarmyhammer/src/toml_config/tests/unit_tests.rs` - Core component tests
   - Tests for ConfigValue enum conversion and serialization  
   - Tests for Configuration struct with nested tables and dot notation
   - Tests for TOML parsing with various valid and invalid inputs
   - Tests for environment variable substitution with different scenarios
   - Tests for error handling and error message quality

2. **Integration Tests** - Test end-to-end workflows
   - `swissarmyhammer/src/toml_config/tests/integration_tests.rs` - End-to-end tests
   - Test configuration loading from file system
   - Test template integration with configuration variables
   - Test workflow execution with configuration variables available
   - Test file discovery from different directory structures

3. **Security and Validation Tests**
   - `swissarmyhammer/src/toml_config/tests/security_tests.rs` - Security validation
   - Test file size limits (1MB maximum)
   - Test depth limits (10 levels maximum)  
   - Test path traversal prevention
   - Test malformed TOML handling
   - Test invalid environment variable syntax

4. **CLI Integration Tests**
   - `swissarmyhammer-cli/tests/config_cli_tests.rs` - CLI command tests
   - Test all CLI configuration management commands
   - Test output formatting (Table, JSON, YAML)
   - Test error scenarios and help messages

### Test Data Strategy

- Create sample sah.toml files with various complexity levels in test data directories
- Generate invalid TOML files for error testing
- Create large configuration files for performance testing
- Set up environment variable test scenarios with proper cleanup

### Property-Based Testing

- Use proptest crate for complex scenarios where applicable
- Test configuration serialization/deserialization round-trips
- Test environment variable substitution with generated inputs
- Test nested structure access with random data

### Implementation Steps

1. Create test module structure and basic test infrastructure
2. Implement unit tests for ConfigValue enum and type conversions
3. Implement unit tests for Configuration struct operations
4. Implement parser tests with valid/invalid TOML inputs
5. Implement environment variable substitution tests
6. Implement security validation tests
7. Implement error scenario tests
8. Implement integration tests for file loading
9. Implement template integration tests
10. Implement CLI integration tests
11. Ensure >90% test coverage across all configuration modules

This comprehensive approach will ensure the sah.toml configuration system is robust, secure, and well-tested across all use cases.