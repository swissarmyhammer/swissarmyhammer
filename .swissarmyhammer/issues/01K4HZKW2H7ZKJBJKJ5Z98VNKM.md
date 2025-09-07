# Complete swissarmyhammer-common Crate for Shared Utilities

## Problem

The `swissarmyhammer-common` crate exists but `swissarmyhammer-tools` still imports common functionality through the main crate:

- `swissarmyhammer::common::rate_limiter::{RateLimiter, RateLimiterConfig, RateLimitChecker, get_rate_limiter}`
- `swissarmyhammer::common::{create_abort_file_current_dir, abort_utils::create_abort_file}`
- `swissarmyhammer::{Result, SwissArmyHammerError}`
- `swissarmyhammer::error::SwissArmyHammerError`

## Solution

Move all common utilities and shared types to `swissarmyhammer-common` crate and make it fully independent.

## Components to Move

### Rate Limiting
- `RateLimiter`, `RateLimiterConfig`, `RateLimitChecker`
- `get_rate_limiter` function

### Abort Utilities  
- `create_abort_file_current_dir`
- `abort_utils::create_abort_file`

### Error Types
- `SwissArmyHammerError` (main error type)
- `Result<T>` type alias
- All error handling utilities

### File Watcher (if applicable)
- `FileWatcher`, `FileWatcherCallback`

## Files Using Common Functionality

- `swissarmyhammer-tools/src/test_utils.rs`
- `swissarmyhammer-tools/src/mcp/server.rs`
- `swissarmyhammer-tools/src/mcp/tool_registry.rs`
- `swissarmyhammer-tools/src/mcp/error_handling.rs`
- `swissarmyhammer-tools/src/mcp/shared_utils.rs`
- `swissarmyhammer-tools/src/mcp/file_watcher.rs`
- Various tool implementations

## Acceptance Criteria

- [ ] All common utilities moved to `swissarmyhammer-common`
- [ ] Error types and `Result` available in common crate
- [ ] Rate limiting functionality fully independent
- [ ] Abort utilities available independently
- [ ] All imports updated to use `swissarmyhammer_common::`
- [ ] All tests pass
- [ ] No dependency on main `swissarmyhammer` crate