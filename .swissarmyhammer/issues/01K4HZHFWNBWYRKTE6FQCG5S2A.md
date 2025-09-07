# Remove git re-export and use swissarmyhammer-git directly

## Problem

Currently, the main `swissarmyhammer` crate re-exports the git functionality:

```rust
// Git operations moved to external crate
pub use swissarmyhammer_git as git;
```

This creates an unnecessary dependency path. Code should reference `swissarmyhammer-git` directly instead of going through the main crate.

## Solution

1. Remove the re-export line from `swissarmyhammer/src/lib.rs`
2. Update all imports from `swissarmyhammer::git::` to `swissarmyhammer_git::`
3. Ensure `swissarmyhammer-git` is properly added to `Cargo.toml` dependencies where needed

## Files to Update

Based on analysis, these files use `swissarmyhammer::git::GitOperations`:

- `swissarmyhammer-tools/src/test_utils.rs`
- `swissarmyhammer-tools/src/mcp/server.rs`
- `swissarmyhammer-tools/src/mcp/tool_registry.rs`
- Various test files throughout the tools crate

## Acceptance Criteria

- [ ] Remove git re-export from main swissarmyhammer crate
- [ ] Update all git imports to use swissarmyhammer-git directly
- [ ] Ensure all affected crates have swissarmyhammer-git dependency
- [ ] All tests pass
- [ ] No compilation errors