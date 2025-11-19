# Step 6: Add Integration Tests for HTTP Endpoints

Refer to .swissarmyhammer/tmp/test-calculator-spec.md

## Goal
Create comprehensive integration tests that verify the calculator API endpoints work correctly end-to-end.

## Requirements
- Test successful addition requests
- Test invalid input handling
- Test error responses and status codes
- Follow existing integration test patterns in the project

## Test Structure

```rust
// File: swissarmyhammer-tools/tests/calculator_http_integration_tests.rs

#[tokio::test]
async fn test_calculator_add_endpoint_success() {
    // Start test server
    // Make request to /calculator/add?a=5&b=3
    // Assert status 200
    // Assert JSON response {"result": 8.0}
}

#[tokio::test]
async fn test_calculator_add_endpoint_invalid_input() {
    // Request with invalid parameters
    // Assert status 400
    // Assert error message in response
}

#[tokio::test]
async fn test_calculator_add_endpoint_missing_params() {
    // Request with missing parameters
    // Assert status 400
}
```

## Test Cases Required

### Happy Path
- Valid positive numbers
- Valid negative numbers  
- Decimal numbers
- Zero values

### Error Cases
- Non-numeric input (a=abc&b=5)
- Missing parameter a
- Missing parameter b
- NaN values
- Infinite values

## Pattern to Follow

Look at `swissarmyhammer-tools/tests/final_http_test.rs` for patterns:
- How to start test HTTP server
- How to make requests
- How to assert responses

## Acceptance Criteria
- [ ] All integration tests pass with `cargo nextest run`
- [ ] Tests cover all specification requirements
- [ ] Tests use real HTTP requests (no mocks)
- [ ] Clear test names and documentation
- [ ] Tests run independently and in parallel
- [ ] No test output pollution (clean logs)

## Dependencies
Requires: Step 5 (routes integrated into server)

## Next Step
Create API documentation with examples
