# Improve graceful process termination with proper signal handling

## Location
`swissarmyhammer-common/src/test_utils.rs:51`

## Current State
```rust
// For now, we'll use a simple approach - just wait a bit then force kill
```

## Description
Process termination currently uses a simple wait-then-kill approach. This should be enhanced with proper signal handling for more graceful termination.

## Requirements
- Implement proper SIGTERM/SIGINT handling
- Add configurable grace period
- Support platform-specific termination signals
- Implement proper cleanup verification
- Add tests for various termination scenarios
- Handle processes that ignore signals

## Platforms
- Unix: SIGTERM, SIGKILL
- Windows: TerminateProcess, WM_CLOSE

## Use Cases
- Test cleanup
- Server shutdown
- Background process management

## Impact
Processes may not clean up properly, leaving zombie processes or leaked resources.