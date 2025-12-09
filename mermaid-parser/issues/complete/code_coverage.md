# Achieve 100% test coverage

## Description
Implement comprehensive test coverage measurement and achieve the 100% coverage goal mentioned in PROJECT_STATUS.md. This ensures all code paths are tested and maintains quality.

## Requirements
1. Setup coverage tooling (cargo-llvm-cov)
2. Measure current coverage baseline
3. Identify uncovered code paths
4. Add tests for missing coverage
5. Integrate coverage into CI/CD

## Setup Instructions
```bash
# Install coverage tool
cargo install cargo-llvm-cov

# Generate coverage report
cargo llvm-cov --html --open

# Generate lcov report for CI
cargo llvm-cov --lcov --output-path lcov.info
```

## Coverage Areas to Focus
- Error handling paths
- Edge cases in parsers
- All AST node types
- Parser recovery scenarios
- Public API surface

## Implementation Plan
1. Run initial coverage report
2. Create tracking issue for each parser below 100%
3. Add tests systematically by module
4. Focus on error conditions and edge cases
5. Ensure examples in docs are tested

## Success Criteria
- 100% line coverage achieved
- 100% branch coverage achieved
- Coverage reports in CI
- Coverage badge in README
- No untested public APIs