# Replace Custom File Watcher Wrapper with Direct notify Crate Usage

## Problem
We have a custom file watcher wrapper in the main `swissarmyhammer` crate that adds unnecessary complexity and coupling. The wrapper is just a thin layer around the popular `notify` crate (version 6) that provides minimal added value while creating dependencies for `swissarmyhammer-tools`.

## Current State
- Main crate has `swissarmyhammer/src/file_watcher.rs` - custom wrapper
- Uses `notify = "6"` crate internally (the standard Rust file watching library)
- `swissarmyhammer-tools` imports: `use swissarmyhammer::file_watcher::{FileWatcher, FileWatcherCallback};`
- Found in files: `file_watcher.rs`, `server.rs`

## Evidence of Usage
```rust
use swissarmyhammer::file_watcher::{FileWatcher, FileWatcherCallback};
```

Used by:
- `src/mcp/file_watcher.rs`
- `src/mcp/server.rs`

## Proposed Solution
**Eliminate the wrapper entirely** and use the `notify` crate directly in `swissarmyhammer-tools`. The wrapper provides no significant value over direct usage.

## Benefits of Removing the Wrapper
- ✅ **Reduces coupling**: One fewer dependency on main crate
- ✅ **Simpler code**: Direct, standard Rust ecosystem patterns
- ✅ **Less maintenance**: No custom wrapper to maintain
- ✅ **More transparent**: Obvious what file watching library is being used
- ✅ **Better control**: Direct access to `notify` features without wrapper limitations

## Implementation Plan

### Phase 1: Analyze Current Wrapper Usage
- [ ] Review `swissarmyhammer/src/file_watcher.rs` to understand wrapper functionality
- [ ] Identify what specific features the wrapper provides
- [ ] Map out how `swissarmyhammer-tools` currently uses the wrapper
- [ ] Determine if any wrapper features are actually necessary

### Phase 2: Add Direct notify Dependency
- [ ] Add `notify = { workspace = true }` to `swissarmyhammer-tools/Cargo.toml`
- [ ] Add `tokio` async integration if needed for file watching
- [ ] Ensure we have the same version as the main workspace

### Phase 3: Replace Wrapper Usage in swissarmyhammer-tools
- [ ] Update `src/mcp/file_watcher.rs`:
  - Remove `use swissarmyhammer::file_watcher::{FileWatcher, FileWatcherCallback};`
  - Add `use notify::{RecommendedWatcher, Watcher, RecursiveMode, Event};`
  - Replace wrapper usage with direct `notify` usage
  - Add minimal tokio async integration if needed
- [ ] Update `src/mcp/server.rs`:
  - Remove file watcher wrapper imports
  - Update file watching initialization to use `notify` directly
  - Maintain same functionality with simpler, direct code

### Phase 4: Implement Direct File Watching
- [ ] Use `notify::RecommendedWatcher` directly instead of wrapper
- [ ] Implement async/tokio integration inline where needed
- [ ] Add minimal configuration inline (no need for wrapper config structs)
- [ ] Ensure error handling works with `notify` errors directly

### Phase 5: Testing and Verification
- [ ] Verify file watching functionality still works correctly
- [ ] Test file watching in MCP server context
- [ ] Ensure no regressions in file watching behavior
- [ ] Run integration tests to verify functionality

### Phase 6: Remove Wrapper from Main Crate (Optional)
- [ ] Check if anything else uses `swissarmyhammer/src/file_watcher.rs`
- [ ] If unused, remove the wrapper entirely from main crate
- [ ] Remove file watcher exports from main crate if no longer needed
- [ ] Clean up any unused dependencies

## Files to Update

### swissarmyhammer-tools
- `Cargo.toml` - Add direct `notify` dependency
- `src/mcp/file_watcher.rs` - Replace wrapper with direct `notify` usage
- `src/mcp/server.rs` - Update file watching initialization

### Optional Main Crate Cleanup
- `src/file_watcher.rs` - Remove if no longer used
- `src/lib.rs` - Remove file watcher exports if no longer needed
- `Cargo.toml` - Potentially remove `notify` dependency if only used for wrapper

## Success Criteria
- [ ] `swissarmyhammer-tools` uses `notify` crate directly
- [ ] No dependency on main crate for file watching
- [ ] File watching functionality preserved and working
- [ ] Simpler, more direct code without wrapper complexity
- [ ] No regressions in file watching behavior
- [ ] Reduced coupling between components

## Risk Mitigation
- Keep wrapper temporarily while implementing direct usage
- Test file watching thoroughly to ensure no behavior changes
- Ensure async/tokio integration works correctly
- Verify error handling with direct `notify` errors
- Test in realistic MCP server scenarios

## Notes
This follows the principle of **using standard ecosystem crates directly** rather than creating unnecessary wrappers. The `notify` crate is mature, well-maintained, and widely used - there's no need for our own abstraction layer.

File watching is not a core domain concern for SwissArmyHammer - it's infrastructure that should use standard tools. Direct usage of `notify` makes the code more transparent and reduces maintenance overhead.

This eliminates another coupling point between `swissarmyhammer-tools` and the main crate, bringing us closer to full domain separation.
## COMPLETION CRITERIA - How to Know This Issue is REALLY Done

**This issue is complete when the following imports NO LONGER EXIST in swissarmyhammer-tools:**

```rust
// These 2+ imports should be ELIMINATED:
use swissarmyhammer::file_watcher::{FileWatcher, FileWatcherCallback};

// Found in these specific locations:
- src/mcp/file_watcher.rs:5
- src/mcp/server.rs:10
```

**And replaced with direct notify usage:**
```rust
use notify::{RecommendedWatcher, Watcher, RecursiveMode, Event};
use tokio::sync::mpsc;
```

**Verification Command:**
```bash
# Should return ZERO results when done:
rg "use swissarmyhammer::file_watcher" swissarmyhammer-tools/

# Should find direct notify usage:
rg "use notify::" swissarmyhammer-tools/
```

**Expected Impact:**
- **Current**: 23 imports from main crate
- **After completion**: 21 imports from main crate (2 file watcher imports eliminated)