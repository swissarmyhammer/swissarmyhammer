# Move Test Utilities from Main Crate to swissarmyhammer-common

## Problem
Test utilities in the main `swissarmyhammer` crate are creating dependencies for `swissarmyhammer-tools` and other components that need testing infrastructure. These utilities are cross-cutting concerns that would be better placed in `swissarmyhammer-common`.

## Current State
Test utilities are currently in `swissarmyhammer/src/test_utils.rs` and are used by:
- `swissarmyhammer-tools` test files
- Various domain crates for testing
- Internal main crate tests

## Evidence of Usage in swissarmyhammer-tools:
```rust
use swissarmyhammer::test_utils::IsolatedTestHome;
```

Found in:
- `src/mcp/tools/issues/work/mod.rs:163`
- `src/mcp/tools/abort/create/mod.rs:97`
- `tests/file_tools_property_tests.rs:14`
- `tests/file_tools_performance_tests.rs:14`

## Proposed Solution
Move test utilities to `swissarmyhammer-common` where they can be shared by all crates without creating circular dependencies.

## Implementation Plan

### Phase 1: Analyze Current Test Utilities
- [ ] Catalog all test utilities in `swissarmyhammer/src/test_utils.rs`
- [ ] Identify which utilities are truly "common" vs main-crate specific
- [ ] Check what domain crates currently use test utilities
- [ ] Map out current test utility dependencies

### Phase 2: Design New Test Utilities Architecture
- [ ] Decide which test utilities belong in `swissarmyhammer-common`
- [ ] Design clean test utility structure for common crate
- [ ] Plan how main-crate specific test utilities will be handled
- [ ] Ensure test utilities have minimal dependencies

### Phase 3: Move Common Test Utilities
- [ ] Create/expand `swissarmyhammer-common/src/test_utils.rs` (may already exist)
- [ ] Move `IsolatedTestHome` to common crate
- [ ] Move other commonly-used test utilities
- [ ] Add proper exports and documentation
- [ ] Ensure common crate has appropriate dev-dependencies

### Phase 4: Update swissarmyhammer-tools
- [ ] Change imports from `swissarmyhammer::test_utils::IsolatedTestHome`
- [ ] To `swissarmyhammer_common::test_utils::IsolatedTestHome`
- [ ] Update all affected files:
  - `src/mcp/tools/issues/work/mod.rs`
  - `src/mcp/tools/abort/create/mod.rs`
  - `tests/file_tools_property_tests.rs`
  - `tests/file_tools_performance_tests.rs`
- [ ] Verify all tests still pass

### Phase 5: Update Domain Crates
- [ ] Update any domain crates using main crate test utilities
- [ ] Add `swissarmyhammer-common` dev-dependency where needed
- [ ] Remove `swissarmyhammer` dev-dependency for test utilities
- [ ] Verify domain crate tests still work

### Phase 6: Update Main Crate
- [ ] Keep main-crate specific test utilities in `swissarmyhammer/src/test_utils.rs`
- [ ] Re-export common test utilities for backward compatibility if needed
- [ ] Update main crate to use common crate for shared test utilities
- [ ] Remove duplicate test utility definitions

### Phase 7: Verification
- [ ] Build entire workspace to ensure no breakage
- [ ] Run all tests to verify test utilities still work
- [ ] Verify domain crates can build and test independently
- [ ] Check that test functionality is preserved
- [ ] Ensure no circular dependencies exist

## Files to Update

### swissarmyhammer-common
- `src/test_utils.rs` - Add common test utilities (may need to create or expand)
- `src/lib.rs` - Export test utilities
- `Cargo.toml` - Add necessary dev-dependencies

### swissarmyhammer-tools (Import Updates)
- `src/mcp/tools/issues/work/mod.rs` - Update test utility imports
- `src/mcp/tools/abort/create/mod.rs` - Update test utility imports
- `tests/file_tools_property_tests.rs` - Update test utility imports
- `tests/file_tools_performance_tests.rs` - Update test utility imports
- `Cargo.toml` - Ensure common crate is available for tests

### Domain Crates
- Update any domain crates using main crate test utilities
- Add `swissarmyhammer-common` dev-dependency
- Remove `swissarmyhammer` dev-dependency for test utilities

### swissarmyhammer (Main Crate)
- `src/test_utils.rs` - Move common utilities out, keep main-specific utilities
- `src/lib.rs` - Update test utility exports
- `Cargo.toml` - Add dev-dependency on common crate if needed

## Success Criteria
- [ ] Common test utilities available in `swissarmyhammer-common`
- [ ] Domain crates and tools don't depend on main crate for test utilities
- [ ] `swissarmyhammer-tools` uses common crate for test utilities
- [ ] All test functionality preserved and working
- [ ] No circular dependencies
- [ ] Workspace builds and all tests pass
- [ ] Reduced coupling between components

## Risk Mitigation
- Start with copying utilities before removing (ensure no breakage)
- Test all affected components thoroughly after each phase
- Maintain backward compatibility during transition
- Keep test behavior and functionality identical
- Plan rollback strategy for each phase

## Benefits
- **Independence**: Components become truly independent for testing
- **Reduced Coupling**: Eliminates test dependency on main crate
- **Consistency**: Shared test utilities across all components
- **Maintainability**: Central location for common test infrastructure

## Notes
Test utilities are cross-cutting concerns similar to error types, making `swissarmyhammer-common` the logical place for them. This will enable domain crates and tools to have comprehensive test coverage without depending on the main crate.

This is part of the broader effort to reduce coupling and enable true domain separation. Test infrastructure should be as independent as production infrastructure.
## COMPLETION CRITERIA - How to Know This Issue is REALLY Done

**This issue is complete when the following imports NO LONGER EXIST in swissarmyhammer-tools:**

```rust
// These 4+ imports should be ELIMINATED:
use swissarmyhammer::test_utils::IsolatedTestHome;

// Found in these specific locations:
- src/mcp/tools/issues/work/mod.rs:163
- src/mcp/tools/abort/create/mod.rs:97
- tests/file_tools_performance_tests.rs:14
- tests/file_tools_property_tests.rs:14
```

**And replaced with:**
```rust
use swissarmyhammer_common::test_utils::IsolatedTestHome;
```

**Verification Command:**
```bash
# Should return ZERO results when done:
rg "use swissarmyhammer::test_utils" swissarmyhammer-tools/

# Should find new imports:
rg "use swissarmyhammer_common::test_utils" swissarmyhammer-tools/
```

**Expected Impact:**
- **Current**: 23 imports from main crate
- **After completion**: ~19 imports from main crate (4 test utility imports eliminated)