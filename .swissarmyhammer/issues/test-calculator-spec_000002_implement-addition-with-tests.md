# Step 2: Implement Addition with Unit Tests

Refer to .swissarmyhammer/tmp/test-calculator-spec.md

## Goal
Implement the addition operation with comprehensive unit tests following TDD principles.

## Requirements
- Implement `add(a: f64, b: f64)` function logic
- Validate inputs are valid numbers (not NaN, not infinite)
- Write unit tests BEFORE completing implementation
- Achieve 100% code coverage

## Test Cases Required

```rust
#[cfg(test)]
mod tests {
    // Happy path
    - test_add_positive_numbers
    - test_add_negative_numbers
    - test_add_zero
    - test_add_decimals
    
    // Error cases
    - test_add_nan_input
    - test_add_infinite_input
}
```

## Implementation Pattern

1. Write failing test for valid addition
2. Implement minimal code to pass
3. Write failing test for NaN validation
4. Add validation logic
5. Write failing test for infinity validation
6. Add validation logic
7. Refactor if needed

## Acceptance Criteria
- [ ] All unit tests pass with `cargo nextest run`
- [ ] Tests cover happy path and error cases
- [ ] Input validation prevents NaN and infinity
- [ ] Clear error messages for validation failures
- [ ] Code formatted with `cargo fmt`
- [ ] No clippy warnings

## Dependencies
Requires: Step 1 (calculator service module structure)

## Next Step
Create HTTP handler types and request/response structures
