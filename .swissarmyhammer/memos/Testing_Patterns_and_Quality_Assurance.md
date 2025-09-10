# Testing Patterns and Quality Assurance

## Test Organization

### Unit Tests
- Place unit tests in `#[cfg(test)]` modules within source files
- Test public API behavior, not implementation details
- Use descriptive test names that explain the scenario
- Follow Arrange-Act-Assert pattern

### Integration Tests
- Place integration tests in `tests/` directory
- Test complete workflows and system interactions
- Use real dependencies, not mocks
- Test error conditions and edge cases

### Property-Based Testing
- Use `proptest` for property-based testing
- Generate diverse test inputs automatically
- Test invariants and properties, not specific outputs
- Useful for parsers, serialization, and mathematical operations

## Test Data Management

### Test Fixtures
- Use builder patterns for test data creation
- Create factories for common test objects
- Use realistic data that represents production scenarios
- Avoid magic numbers and hardcoded strings

### Database Testing
- Use separate test databases or in-memory databases
- Reset database state between tests
- Use transactions that rollback for isolation
- Test database migrations and schema changes

### File System Testing
- Use temporary directories for file operations
- Clean up test files in teardown
- Test with different file permissions and ownership
- Use `tempfile` crate for temporary file management

## Test Doubles

### When to Mock
- Mock external services and APIs
- Mock slow or unreliable dependencies
- Never mock internal code or value objects
- Use real implementations for databases and file systems when possible

### Mock Implementation
- Use `mockall` crate for trait-based mocking
- Keep mocks simple and focused
- Verify mock interactions when behavior matters
- Prefer fakes over mocks when possible

## Quality Metrics

### Code Coverage
- Aim for high line coverage (>80%) but focus on branch coverage
- Use `cargo tarpaulin` for coverage reporting
- Don't chase 100% coverage at the expense of test quality
- Identify uncovered code paths and add targeted tests

### Performance Testing
- Only add performance tests when explicitly requested
- Use `criterion` crate for microbenchmarks
- Test realistic workloads, not synthetic scenarios
- Set performance regression thresholds

### Mutation Testing
- Consider mutation testing for critical algorithms
- Use `cargo-mutants` for mutation testing
- Focus on areas with complex logic
- Use results to improve test quality, not just coverage

## Test Environment

### Test Configuration
- Use separate configuration for tests
- Override default timeouts for faster test execution
- Disable unnecessary features in test builds
- Use feature flags to control test behavior

### Continuous Integration
- Run tests on multiple platforms and Rust versions
- Use matrix builds for different feature combinations
- Fail fast on test failures
- Cache dependencies for faster builds

### Test Documentation
- Document test setup requirements
- Explain complex test scenarios
- Maintain test data documentation
- Keep test documentation up to date with code changes