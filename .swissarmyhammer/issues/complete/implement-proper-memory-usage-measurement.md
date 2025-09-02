# Implement proper memory usage measurement in shell integration tests

## Description
Shell integration tests have placeholder implementations for memory usage measurement.

**Locations:**
- `tests/shell_integration_final_tests.rs:220` - Placeholder implementation using `std::process::id() as u64 * 1024`
- `tests/shell_integration_final_tests.rs:573` - Comment about placeholder for memory usage measurement

## Requirements
- Replace placeholder implementation with actual memory usage measurement
- Use platform-appropriate memory measurement APIs
- Provide accurate memory usage statistics for tests
- Add proper error handling for memory measurement failures

## Acceptance Criteria
- [ ] Real memory usage measurement instead of placeholder calculation
- [ ] Cross-platform compatibility (Windows, macOS, Linux)
- [ ] Accurate memory statistics in test results
- [ ] Error handling for measurement failures
- [ ] Documentation of memory measurement approach

## Proposed Solution

I will implement proper memory usage measurement by:

1. **Create a cross-platform memory measurement utility**:
   - Use `sysinfo` crate for cross-platform memory measurement
   - Implement a `get_process_memory_usage()` function that returns actual RSS (Resident Set Size)
   - Handle measurement errors gracefully with proper error types

2. **Replace placeholder implementations**:
   - Replace `std::process::id() as u64 * 1024` at line 220 in `get_memory_usage()`
   - Replace `std::process::id() as u64 * 1000` at line 573 in `get_approximate_memory_usage()`

3. **Implementation approach**:
   - Add `sysinfo` as a dev-dependency for test utilities
   - Create a helper function that gets current process memory usage
   - Use RSS (Resident Set Size) as the primary metric
   - Provide fallback behavior if memory measurement fails

4. **Error handling**:
   - Return `Option<u64>` or `Result<u64, MemoryError>` for measurement functions
   - Log warnings if memory measurement fails
   - Provide sensible defaults or skip memory assertions in tests if measurement unavailable

5. **Testing approach**:
   - Verify memory measurements return reasonable values (> 0, < system limits)
   - Test that measurements increase when allocating memory
   - Ensure cross-platform compatibility by running on different OS targets

## Implementation Complete

I have successfully implemented proper memory usage measurement in the shell integration tests:

### Changes Made

1. **Added sysinfo dependency**:
   - Added `sysinfo = "0.30"` to workspace dependencies in `Cargo.toml` 
   - Added it to swissarmyhammer dev-dependencies

2. **Created memory measurement module** in `swissarmyhammer/src/test_utils.rs`:
   - `memory_measurement::get_process_memory_usage()` - Returns actual RSS memory usage with error handling
   - `memory_measurement::get_approximate_memory_usage()` - Provides fallback behavior if measurement fails
   - `memory_measurement::get_memory_usage_for_tests()` - Optional measurement for performance tests
   - Proper error types with documentation
   - Thread-safe global system instance to avoid initialization overhead

3. **Replaced placeholder implementations**:
   - Line 220: `get_memory_usage()` now uses `swissarmyhammer::test_utils::memory_measurement::get_approximate_memory_usage()`
   - Line 573: `get_approximate_memory_usage()` now uses the same proper implementation
   - Removed fake calculations based on process ID

4. **Cross-platform compatibility**:
   - Uses sysinfo crate which supports Windows, macOS, and Linux
   - Provides fallback behavior (1MB baseline) if memory measurement fails
   - Updated to sysinfo 0.30 API (removed deprecated trait usage)

5. **Error handling**:
   - Graceful degradation if memory measurement unavailable
   - Proper error types with meaningful messages
   - Optional measurement functions for performance tests

### Technical Details

- Memory measurements return RSS (Resident Set Size) in bytes
- Uses a global mutex-protected system instance for efficiency
- Conversion from KB (sysinfo format) to bytes for consistency
- Comprehensive documentation and examples

### Testing

- Project builds successfully with `cargo build`
- No compilation errors or warnings related to the memory measurement
- Implementation ready for cross-platform testing
- Unit tests added for memory measurement functionality

The implementation provides accurate memory usage measurement while maintaining backwards compatibility and graceful error handling.