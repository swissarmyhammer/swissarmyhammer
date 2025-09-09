# Replace Custom File Watcher Implementation with async-watcher Crate

## Problem
Even after eliminating the file watcher wrapper from swissarmyhammer-tools dependencies (commit 56da9d05), we still have custom file watching implementation code. We should use the `async-watcher` crate instead of maintaining any custom file watching logic.

## Current State
- ✅ File watcher wrapper dependency eliminated from swissarmyhammer-tools
- ❌ Still have custom file watching implementation code
- ❌ Maintaining file watching logic when ecosystem solutions exist

## Research Finding
The `async-watcher` crate provides exactly what we need:

**Features:**
- ✅ **Built on notify**: Uses standard `notify` crate underneath
- ✅ **Async/Tokio**: Native async interface designed for tokio
- ✅ **Debounced**: Prevents excessive file change events (better than raw notify)
- ✅ **Simple API**: Easy to use and integrate
- ✅ **Recent**: Updated May 2024, actively maintained
- ✅ **MIT Licensed**: Compatible with our project

**Usage Example:**
```rust
use async_watcher::{Watcher, AsyncWatcher};
use tokio::sync::mpsc;
use std::time::Duration;

let (tx, rx) = mpsc::channel(100);
let watcher = AsyncWatcher::new(tx, Duration::from_millis(500)).await?;
watcher.watch("/path/to/dir", RecursiveMode::Recursive)?;

// Handle events from rx channel
```

## Proposed Solution
**Replace ALL custom file watching code** with the `async-watcher` crate.

## Implementation Plan

### Phase 1: Add async-watcher Dependency
- [ ] Add `async-watcher = "0.3"` to workspace Cargo.toml
- [ ] Add dependency to swissarmyhammer-tools if file watching is needed there
- [ ] Research async-watcher API and integration patterns

### Phase 2: Identify Current Custom File Watching Code
- [ ] Find all remaining custom file watching implementation code
- [ ] Review commit 56da9d05 to see what file watching code still exists
- [ ] Map current file watching functionality to async-watcher equivalents
- [ ] Identify any unique features that need preservation

### Phase 3: Replace Custom Implementation
- [ ] Replace custom file watching logic with async-watcher usage
- [ ] Use async-watcher's debounced event handling
- [ ] Implement file change handlers using async-watcher's async traits
- [ ] Remove all custom file watching implementation code
- [ ] Remove notify direct usage in favor of async-watcher

### Phase 4: Update Integration Points
- [ ] Update any MCP server file watching to use async-watcher
- [ ] Update any prompt directory watching to use async-watcher
- [ ] Ensure file change notifications still work correctly
- [ ] Test file watching behavior with async-watcher

### Phase 5: Clean Up Dependencies
- [ ] Remove direct `notify` dependency if only used for file watching
- [ ] Remove any custom file watching utilities or helpers
- [ ] Clean up any unused tokio channel code for file watching
- [ ] Simplify file watching configuration

### Phase 6: Testing and Verification
- [ ] Test file watching behavior works exactly as before
- [ ] Test debouncing prevents excessive events
- [ ] Verify async integration works correctly
- [ ] Test file watching performance and resource usage
- [ ] Ensure no regressions in file change detection

## Benefits of async-watcher

### **Vs Custom Implementation:**
- ✅ **Zero maintenance**: No file watching code to maintain
- ✅ **Better debouncing**: Built-in debouncing prevents excessive events
- ✅ **Async-first**: Designed specifically for tokio applications
- ✅ **Battle-tested**: Used by other projects, tested implementation
- ✅ **Simple integration**: Much simpler than custom notify + tokio setup

### **Vs Direct notify:**
- ✅ **Async interface**: No manual tokio channel setup needed
- ✅ **Debouncing**: Prevents event flooding on file changes
- ✅ **Cleaner API**: Purpose-built for async file watching use cases

## Files to Update

### Add async-watcher Usage
- Replace any remaining custom file watching code
- Update MCP server file watching integration
- Update prompt directory watching

### Remove Custom Implementation
- Delete any remaining custom file watching utilities
- Remove direct notify usage for file watching
- Clean up custom tokio channel handling

## Expected Code Reduction

### Before (Custom):
```rust
// Complex custom file watching setup
use notify::{RecommendedWatcher, Watcher, RecursiveMode, Event};
use tokio::sync::mpsc;
// + custom debouncing logic
// + custom async integration  
// + custom error handling
```

### After (async-watcher):
```rust
use async_watcher::{AsyncWatcher, Watcher};
use std::time::Duration;

let watcher = AsyncWatcher::new(tx, Duration::from_millis(500)).await?;
watcher.watch("/path", RecursiveMode::Recursive)?;
```

## Success Criteria
- [ ] Zero custom file watching implementation code
- [ ] All file watching uses async-watcher crate
- [ ] File change detection works exactly as before
- [ ] Better debouncing behavior (fewer excessive events)
- [ ] Simpler, more maintainable file watching code
- [ ] All tests pass

## Risk Mitigation
- Test file watching behavior thoroughly
- Ensure debouncing time is appropriate for use cases
- Verify async-watcher handles all file events we need
- Test performance compared to custom implementation
- Keep rollback option available

## Notes
This completely eliminates file watching as a maintenance concern by using a purpose-built crate. The `async-watcher` crate is exactly designed for our use case: tokio-based applications that need debounced file change notifications.

Using async-watcher follows our principle of **using ecosystem standards** rather than maintaining custom infrastructure code. File watching is solved problem - we should use the async-first solution designed for tokio applications.