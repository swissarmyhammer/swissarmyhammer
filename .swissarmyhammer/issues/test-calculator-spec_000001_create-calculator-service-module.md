# Step 1: Create Calculator Service Module

Refer to .swissarmyhammer/tmp/test-calculator-spec.md

## Goal
Create the core calculator service module with basic addition functionality and proper error handling.

## Requirements
- Create `swissarmyhammer-calculator` crate following workspace patterns
- Implement `add(a: f64, b: f64) -> Result<f64, CalculatorError>` function
- Define `CalculatorError` enum for validation errors
- Follow Rust language patterns from memos

## Implementation Details

```rust
// Structure
swissarmyhammer-calculator/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── error.rs
    └── operations.rs
```

### Error Types Needed
- InvalidInput - for non-numeric or invalid values
- OperationError - for calculation failures

### Key Patterns
- Use `Result<T, E>` for all operations
- Implement proper error messages
- No unwrap() or panic!()
- Add comprehensive doc comments

## Acceptance Criteria
- [ ] Crate compiles with `cargo build`
- [ ] Module structure follows workspace conventions
- [ ] Error types properly defined
- [ ] Core addition function signature defined (implementation in next step)
- [ ] All code formatted with `cargo fmt`
- [ ] No clippy warnings with `cargo clippy`

## Dependencies
None - this is the first step

## Next Step
Implement the addition logic and unit tests
