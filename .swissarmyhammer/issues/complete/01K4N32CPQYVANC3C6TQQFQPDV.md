# Complete Error Type Migration to swissarmyhammer-common - Finish Issue 01K4MGJQSZ2ZGRRJR1Q6K4HNQE

## Problem
Issue `01K4MGJQSZ2ZGRRJR1Q6K4HNQE` is marked as complete but the error type migration was never actually implemented. swissarmyhammer-tools still imports error types from the main crate instead of using swissarmyhammer-common.

## Evidence of Incomplete Implementation
Despite the issue being marked complete, the following imports still exist in swissarmyhammer-tools:

```rust
// These should NOT exist if the migration was complete:
use swissarmyhammer::{Result, SwissArmyHammerError};           // 4+ files
use swissarmyhammer::error::SwissArmyHammerError;              // 1+ file

// Found in:
- src/mcp/error_handling.rs:4
- src/mcp/file_watcher.rs:6  
- src/mcp/shared_utils.rs:8
- src/mcp/server.rs:15
- src/mcp/tool_handlers.rs:9
```

## Current State Analysis

### swissarmyhammer-common Already Has Errors
`swissarmyhammer-common/src/error.rs` already exists with:
```rust
pub type Result<T> = std::result::Result<T, SwissArmyHammerError>;

#[derive(Debug, ThisError)]
pub enum SwissArmyHammerError {
    Io(#[from] io::Error),
    Serialization(#[from] serde_yaml::Error),
    // ... other error variants
}
```

### Main Crate Still Has Errors
`swissarmyhammer/src/error.rs` likely still exists with duplicate/similar error types.

### Tools Import From Wrong Place
swissarmyhammer-tools imports errors from main crate instead of common crate.

## Specific Implementation Tasks

### Phase 1: Verify Common Crate Error Completeness
- [ ] Review `swissarmyhammer-common/src/error.rs` 
- [ ] Compare with `swissarmyhammer/src/error.rs`
- [ ] Identify any missing error variants in common crate
- [ ] Add any missing core error types to common crate
- [ ] Ensure `Result<T>` type alias is properly exported

### Phase 2: Update swissarmyhammer-tools Imports
- [ ] **Update `src/mcp/error_handling.rs:4`**:
  ```rust
  // FROM: use swissarmyhammer::{PromptLibrary, PromptResolver, Result, SwissArmyHammerError};
  // TO:   use swissarmyhammer::{PromptLibrary, PromptResolver};
  //       use swissarmyhammer_common::{Result, SwissArmyHammerError};
  ```

- [ ] **Update `src/mcp/file_watcher.rs:6`**:
  ```rust
  // FROM: use swissarmyhammer::{Result, SwissArmyHammerError};
  // TO:   use swissarmyhammer_common::{Result, SwissArmyHammerError};
  ```

- [ ] **Update `src/mcp/shared_utils.rs:8`**:
  ```rust
  // FROM: use swissarmyhammer::{Result, SwissArmyHammerError};
  // TO:   use swissarmyhammer_common::{Result, SwissArmyHammerError};
  ```

- [ ] **Update `src/mcp/server.rs:15`**:
  ```rust
  // FROM: use swissarmyhammer::{PromptLibrary, PromptResolver, Result, SwissArmyHammerError};
  // TO:   use swissarmyhammer::{PromptLibrary, PromptResolver};
  //       use swissarmyhammer_common::{Result, SwissArmyHammerError};
  ```

- [ ] **Update `src/mcp/tool_handlers.rs:9`**:
  ```rust
  // FROM: use swissarmyhammer::error::SwissArmyHammerError;
  // TO:   use swissarmyhammer_common::SwissArmyHammerError;
  ```

### Phase 3: Verify Cargo.toml Dependencies
- [ ] Ensure `swissarmyhammer-tools/Cargo.toml` has:
  ```toml
  swissarmyhammer-common = { path = "../swissarmyhammer-common" }
  ```
- [ ] Verify swissarmyhammer-common is available for import

### Phase 4: Handle Error Conversions
- [ ] Check for any error conversions that need updating
- [ ] Ensure domain-specific errors still convert to common errors properly
- [ ] Update any error handling code that depends on specific error variants
- [ ] Test error propagation and conversion chains

### Phase 5: Test and Verify
- [ ] Build swissarmyhammer-tools to ensure no compilation errors
- [ ] Run tests to verify error handling still works
- [ ] Test error message formatting and display
- [ ] Verify error context and tracing still works
- [ ] Check MCP error responses are still correct

### Phase 6: Clean Up Main Crate (Optional)
- [ ] Check if main crate still needs its own error types
- [ ] Consider re-exporting common errors from main crate if needed for backward compatibility
- [ ] Update main crate to use common errors where appropriate

## Success Criteria
- [ ] swissarmyhammer-tools imports NO error types from main crate
- [ ] ALL error imports come from swissarmyhammer-common  
- [ ] Build succeeds without compilation errors
- [ ] All tests pass
- [ ] Error handling behavior is preserved
- [ ] MCP error responses work correctly

## Verification Commands
```bash
# Should return ZERO results:
rg "use swissarmyhammer::(.*)?Result|SwissArmyHammerError" swissarmyhammer-tools/

# Should find the new imports:
rg "use swissarmyhammer_common::(.*)?Result|SwissArmyHammerError" swissarmyhammer-tools/

# Build test:
cd swissarmyhammer-tools && cargo build
```

## Files to Update
- `swissarmyhammer-tools/src/mcp/error_handling.rs`
- `swissarmyhammer-tools/src/mcp/file_watcher.rs`
- `swissarmyhammer-tools/src/mcp/shared_utils.rs`
- `swissarmyhammer-tools/src/mcp/server.rs`
- `swissarmyhammer-tools/src/mcp/tool_handlers.rs`

## Expected Impact
This will eliminate 5+ import dependencies from swissarmyhammer-tools to the main crate, reducing coupling significantly. After completion:

**Before**: 23 imports from main crate  
**After**: ~18 imports from main crate (5 error imports eliminated)

## Notes
This issue is marked as complete but clearly wasn't implemented. The migration should be straightforward since:
- swissarmyhammer-common already has error types
- The imports just need to be updated
- No major code restructuring should be needed

This is a critical foundation for other domain separations since error handling is cross-cutting.
## Proposed Solution

After analyzing both error implementations, the migration path is clear:

### Analysis Summary

**swissarmyhammer-common/src/error.rs** has:
- Complete core error infrastructure 
- `SwissArmyHammerError` enum with common variants (IO, JSON, YAML, etc.)
- `Result<T>` type alias
- Comprehensive error utilities (ErrorContext, ErrorChain, etc.)
- All necessary infrastructure errors

**swissarmyhammer/src/error.rs** has:
- Domain-specific errors plus duplicated infrastructure errors
- Already imports and re-exports common errors
- Has redundant variants that should delegate to common crate

**swissarmyhammer-tools** currently imports from the main crate instead of common crate.

### Implementation Steps

1. **Update all swissarmyhammer-tools imports** to use `swissarmyhammer_common` instead of `swissarmyhammer` for error types
2. **Files to update** (5 files total):
   - `src/mcp/error_handling.rs:4`
   - `src/mcp/file_watcher.rs:7`
   - `src/mcp/shared_utils.rs:8`
   - `src/mcp/server.rs:16`
   - `src/mcp/tool_handlers.rs:9`

3. **Verify Cargo.toml** already has swissarmyhammer-common dependency
4. **Build and test** to ensure no compilation errors
5. **Verify** no imports remain from main crate

### Expected Impact
- Reduces coupling between swissarmyhammer-tools and main crate by 5+ imports
- Aligns with architectural goal of using common crate for shared infrastructure
- No functional changes - just import source changes

The fix is straightforward since swissarmyhammer-common already has all needed error types.