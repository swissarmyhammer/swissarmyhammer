# Step 4: Implement HTTP Handlers

Refer to .swissarmyhammer/tmp/test-calculator-spec.md

## Goal
Create axum HTTP handlers for the `/add` endpoint with proper validation and error handling.

## Requirements
- Implement handler function for `/add` endpoint
- Parse query parameters `a` and `b`
- Validate inputs are valid numbers
- Return JSON response with result or error
- Use proper HTTP status codes (200, 400)

## Handler Implementation

```rust
use axum::{
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Json},
};

pub async fn add_handler(
    Query(params): Query<AddQuery>,
) -> impl IntoResponse {
    // 1. Parse string inputs to f64
    // 2. Validate both are valid numbers
    // 3. Call calculator service
    // 4. Return appropriate response
}
```

## Error Handling Pattern

```rust
// Return (StatusCode, Json<ErrorResponse>) for errors
// Return (StatusCode, Json<CalculatorResponse>) for success
```

## Acceptance Criteria
- [ ] Handler function compiles
- [ ] Proper input parsing from strings to numbers
- [ ] HTTP 400 returned for invalid inputs
- [ ] HTTP 200 returned for successful operations
- [ ] JSON responses match specification format
- [ ] Clear error messages in responses
- [ ] Unit tests for handler logic (mock Request)

## Dependencies
Requires: Step 3 (HTTP types defined)

## Next Step
Integrate handlers into existing axum router
