Using Ulid::new() is apparently wrong. Look at https://docs.rs/ulid/latest/ulid/struct.Generator.html

Make sure all Ulid allocated are monotonic.

Using Ulid::new() is apparently wrong. Look at https://docs.rs/ulid/latest/ulid/struct.Generator.html

Make sure all Ulid allocated are monotonic.

## Proposed Solution

After reviewing the ULID documentation, I need to replace all uses of `Ulid::new()` with proper monotonic generation using a `Generator`. The key issues:

1. **Current Problem**: `Ulid::new()` doesn't guarantee monotonic ordering - multiple calls could produce the same or non-ordered ULIDs
2. **Proper Solution**: Use `ulid::Generator` which ensures each call produces a larger ULID value than the previous call

### Implementation Steps:

1. **Create Centralized ULID Generation**: Create a thread-safe global generator or utility function
2. **Replace All `Ulid::new()` Calls**: Found in:
   - `swissarmyhammer/src/memoranda/mod.rs:488` - `MemoId::new()`
   - `swissarmyhammer/src/workflow/run.rs:15` - `WorkflowRunId::new()`  
   - `swissarmyhammer/src/issues/filesystem.rs` - likely in issue ID generation

3. **Add Tests**: Verify monotonic behavior with property-based tests
4. **Update Documentation**: Update examples and docs to show proper usage

### Technical Approach:
- Use `std::sync::OnceLock` or similar for thread-safe global generator
- Wrap generator in mutex for thread safety
- Create utility functions that hide implementation details
- Consider using a per-thread generator pattern for better performance