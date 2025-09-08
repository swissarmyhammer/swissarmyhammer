# Complete swissarmyhammer-memoranda Domain Crate Migration

## Problem
The migration from `swissarmyhammer/src/memoranda` to the `swissarmyhammer-memoranda` domain crate was started but never completed. Currently both exist and swissarmyhammer-tools is still using the old main crate imports instead of the domain crate.

## Current State Analysis

### What Exists (✅)
- `swissarmyhammer-memoranda` domain crate is complete and functional
- Domain crate has proper error handling, storage, and operations

### What's Wrong (❌)
- `swissarmyhammer-tools` still imports from main crate:
  ```rust
  use swissarmyhammer::memoranda::{MarkdownMemoStorage, MemoStorage};
  ```
- Should be using domain crate:
  ```rust
  use swissarmyhammer_memoranda::{MarkdownMemoStorage, MemoStorage};
  ```
- `swissarmyhammer/src/memoranda/` likely still exists with duplicate code

## Evidence of Incomplete Migration

### Current Wrong Usage in swissarmyhammer-tools:
- `tests/file_tools_integration_tests.rs:12: use swissarmyhammer::memoranda::{MarkdownMemoStorage, MemoStorage};`
- `tests/notify_integration_tests.rs:9: use swissarmyhammer::memoranda::{MarkdownMemoStorage, MemoStorage};`
- `tests/file_tools_performance_tests.rs:13: use swissarmyhammer::memoranda::{MarkdownMemoStorage, MemoStorage};`
- `tests/file_tools_property_tests.rs:13: use swissarmyhammer::memoranda::{MarkdownMemoStorage, MemoStorage};`
- `tests/test_issue_show_enhanced.rs:9: use swissarmyhammer::memoranda::{MarkdownMemoStorage, MemoStorage};`

### What Should Happen:
All imports should use the domain crate: `swissarmyhammer_memoranda`

## Implementation Plan

### Phase 1: Verify Domain Crate is Ready
- [ ] Confirm `swissarmyhammer-memoranda` has all necessary functionality
- [ ] Verify it exports `MarkdownMemoStorage` and `MemoStorage` 
- [ ] Check that it has the same API as the main crate version
- [ ] Run domain crate tests to ensure functionality

### Phase 2: Update swissarmyhammer-tools Dependencies
- [ ] Ensure `swissarmyhammer-memoranda` is in `swissarmyhammer-tools/Cargo.toml` (likely already there)
- [ ] Update all imports in swissarmyhammer-tools:
  - `tests/file_tools_integration_tests.rs`
  - `tests/notify_integration_tests.rs`  
  - `tests/file_tools_performance_tests.rs`
  - `tests/file_tools_property_tests.rs`
  - `tests/test_issue_show_enhanced.rs`
- [ ] Update any other files using memoranda imports
- [ ] Verify all tests pass with new imports

### Phase 3: Remove Old Memoranda Code
- [ ] Check if `swissarmyhammer/src/memoranda/` exists (likely does)
- [ ] Remove `swissarmyhammer/src/memoranda/` directory entirely if it exists
- [ ] Update `swissarmyhammer/src/lib.rs` to remove memoranda module exports
- [ ] Remove any memoranda-related re-exports from main crate

### Phase 4: Clean Up Dependencies
- [ ] Verify main crate doesn't have `swissarmyhammer-memoranda` as dependency
- [ ] Remove any circular dependencies if they exist
- [ ] Check that main crate no longer depends on its own domain crate

### Phase 5: Verification
- [ ] Build entire workspace to ensure no breakage
- [ ] Run all tests, especially memoranda-related functionality
- [ ] Verify memo creation, listing, retrieval still works through MCP tools
- [ ] Check that swissarmyhammer-tools can build independently
- [ ] Ensure no circular dependencies exist

## Files to Update

### swissarmyhammer-tools (Import Updates)
- `tests/file_tools_integration_tests.rs` - Change import to domain crate
- `tests/notify_integration_tests.rs` - Change import to domain crate
- `tests/file_tools_performance_tests.rs` - Change import to domain crate
- `tests/file_tools_property_tests.rs` - Change import to domain crate
- `tests/test_issue_show_enhanced.rs` - Change import to domain crate

### swissarmyhammer (Cleanup - if memoranda exists)
- Remove `src/memoranda/` directory entirely if it exists
- Remove from `src/lib.rs`: Any memoranda module exports
- Remove any memoranda dependencies from main crate

## Success Criteria
- [ ] `swissarmyhammer-tools` uses `swissarmyhammer_memoranda` domain crate exclusively
- [ ] `swissarmyhammer/src/memoranda/` no longer exists (if it existed)
- [ ] All memo functionality continues to work through MCP tools
- [ ] No circular dependencies between main crate and domain crate
- [ ] Workspace builds and tests pass
- [ ] Reduced coupling between swissarmyhammer-tools and main crate

## Risk Mitigation
- Verify API compatibility between old and new memoranda modules
- Test thoroughly after import changes
- Keep git commits granular for easy rollback
- Check for any undocumented dependencies on main crate memoranda

## Notes
This follows the same pattern as the search and issues migrations - domain crate was created but tools were never updated to use it, and old code was never removed. This is part of the broader effort to reduce dependencies from swissarmyhammer-tools on the main swissarmyhammer crate.