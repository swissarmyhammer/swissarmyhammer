# Implement Unit Tests for Notify Tool

Refer to /Users/wballard/github/swissarmyhammer/ideas/notify_tool.md

## Objective
Create comprehensive unit tests for the NotifyTool covering parameter validation, response formatting, and core functionality.

## Tasks
1. Create test module in `notify/create/mod.rs`
2. Test parameter validation scenarios
3. Test response formatting
4. Test logging integration with different levels
5. Test error handling scenarios
6. Test structured context handling

## Test Coverage Requirements

### Parameter Validation Tests
- Test empty message validation (should fail)
- Test valid message acceptance
- Test level validation (info, warn, error)
- Test invalid level handling (should default to info)
- Test context parameter handling (optional)

### Response Format Tests
- Verify correct response structure
- Test response message formatting
- Ensure `is_error: false` for successful operations
- Test error responses for validation failures

### Logging Integration Tests
- Mock tracing to verify correct target usage
- Test all three log levels (info, warn, error)
- Verify context data is properly included
- Test fallback to info level for invalid levels

### Error Handling Tests  
- Test empty message error case
- Test malformed JSON context
- Test logging failure scenarios
- Verify graceful error handling

## Implementation Notes
- Follow testing patterns from existing MCP tools
- Use `#[cfg(test)]` module organization
- Include mock tracing setup for testing log calls
- Use proper async test setup with `#[tokio::test]`
- Test both success and failure scenarios comprehensively

## Success Criteria
- All test scenarios pass
- Code coverage includes all major code paths
- Tests follow established patterns from other tools
- Mock tracing verifies correct logging behavior
- Error scenarios are properly tested

## Dependencies
- Build on documentation from step 000006