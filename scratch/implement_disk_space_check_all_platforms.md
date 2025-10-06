# Implement disk space checking for all platforms

## Location
`swissarmyhammer-cli/src/commands/doctor/utils.rs:218`

## Current State
```rust
"Disk space checking is not implemented for this platform. \
```

## Description
The `doctor` command's disk space check is not implemented for certain platforms. This should be implemented to provide consistent diagnostics across all supported platforms.

## Requirements
- Implement disk space checking for Windows
- Implement disk space checking for macOS (if missing)
- Implement disk space checking for Linux (if missing)
- Use platform-appropriate APIs
- Handle errors gracefully
- Add platform-specific tests

## Impact
Incomplete diagnostic information on some platforms.