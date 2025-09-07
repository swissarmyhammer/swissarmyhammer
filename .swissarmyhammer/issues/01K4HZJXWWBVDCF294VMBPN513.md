# Extract Git Operations to Independent swissarmyhammer-git Crate

## Problem

The `swissarmyhammer-git` crate exists but may not be complete and independent. Currently, `swissarmyhammer-tools` imports `swissarmyhammer::git::GitOperations`, indicating the git functionality is still tied to the main crate.

## Solution

Ensure `swissarmyhammer-git` is a complete, standalone crate that can be used without depending on the main `swissarmyhammer` crate.

## Tasks

1. Review current `swissarmyhammer-git` crate completeness
2. Move any remaining git functionality from main crate to `swissarmyhammer-git`
3. Ensure `GitOperations` trait and implementations are fully contained
4. Update dependencies to use `swissarmyhammer-git` directly
5. Remove git module from main `swissarmyhammer` crate

## Files Using Git Operations

- `swissarmyhammer-tools/src/test_utils.rs`
- `swissarmyhammer-tools/src/mcp/server.rs` 
- `swissarmyhammer-tools/src/mcp/tool_registry.rs`
- Various test files

## Acceptance Criteria

- [ ] `swissarmyhammer-git` crate is fully independent
- [ ] All git operations work without main crate dependency
- [ ] All imports updated to use `swissarmyhammer_git::` directly
- [ ] All tests pass
- [ ] No circular dependencies