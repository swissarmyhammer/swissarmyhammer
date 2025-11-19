# Step 3: Create HTTP Types and Structures

Refer to .swissarmyhammer/tmp/test-calculator-spec.md

## Goal
Define the HTTP request/response types for the calculator API endpoints.

## Requirements
- Create structures for query parameters
- Create structures for JSON responses
- Implement serialization/deserialization
- Define error response format

## Types to Define

```rust
// Query parameters for /add endpoint
#[derive(Debug, Deserialize)]
pub struct AddQuery {
    pub a: String,  // String to handle validation
    pub b: String,
}

// Success response
#[derive(Debug, Serialize)]
pub struct CalculatorResponse {
    pub result: f64,
}

// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub status: u16,
}
```

## Dependencies Needed
- `serde` with derive feature
- `serde_json` for JSON serialization

## Acceptance Criteria
- [ ] All types defined with proper derives
- [ ] Serialization/deserialization working
- [ ] Unit tests for type conversion
- [ ] Doc comments on all public types
- [ ] Code compiles and passes tests
- [ ] No clippy warnings

## Dependencies
Requires: Step 2 (calculator operations implemented)

## Next Step
Create HTTP handler functions for the endpoints
