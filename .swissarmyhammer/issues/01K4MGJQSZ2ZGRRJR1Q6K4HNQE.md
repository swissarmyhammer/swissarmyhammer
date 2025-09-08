# Move Error Types from Main Crate to swissarmyhammer-common

## Problem
Currently, all domain crates and swissarmyhammer-tools depend on error types from the main `swissarmyhammer` crate, creating a major coupling point that prevents full independence:

```rust
use swissarmyhammer::{Result, SwissArmyHammerError};
```

This creates a circular dependency problem where domain crates that should be independent still need the main crate for basic error handling.

## Current State Analysis

### Major Dependencies on Main Crate Errors
- **swissarmyhammer-tools** - Extensively uses `SwissArmyHammerError` and `Result`
- **Domain crates** - Many likely depend on main crate error types
- **Error conversion** - Tools convert between domain errors and main crate errors

### Evidence from swissarmyhammer-tools:
```rust
// Current problematic imports:
use swissarmyhammer::{Result, SwissArmyHammerError};
use swissarmyhammer::error::SwissArmyHammerError;

// Found in files:
- src/mcp/error_handling.rs
- src/mcp/file_watcher.rs  
- src/mcp/shared_utils.rs
- src/mcp/server.rs
- src/mcp/tool_handlers.rs
- Multiple other files
```

## Proposed Solution
Move core error types to `swissarmyhammer-common` where they can be shared by all crates without creating circular dependencies.

## Implementation Plan

### Phase 1: Analyze Current Error Usage
- [ ] Catalog all error types currently in `swissarmyhammer/src/error.rs`
- [ ] Identify which errors are truly "common" vs domain-specific
- [ ] Check what domain crates currently depend on main crate errors
- [ ] Map out current error conversion patterns

### Phase 2: Design New Error Architecture  
- [ ] Decide which errors belong in `swissarmyhammer-common`
- [ ] Design clean error hierarchy for common errors
- [ ] Plan how domain-specific errors will convert to common errors
- [ ] Ensure `Result<T>` type can be in common crate

### Phase 3: Move Core Errors to Common Crate
- [ ] Create new error types in `swissarmyhammer-common/src/error.rs`
- [ ] Move core `SwissArmyHammerError` enum to common crate
- [ ] Move `Result<T>` type alias to common crate  
- [ ] Add proper error conversion traits
- [ ] Ensure common crate has minimal dependencies

### Phase 4: Update Domain Crates
- [ ] Update domain crates to use `swissarmyhammer-common` errors
- [ ] Remove dependencies on main crate for errors
- [ ] Add proper error conversions from domain errors to common errors
- [ ] Update domain crate `Cargo.toml` files

### Phase 5: Update swissarmyhammer-tools
- [ ] Change imports from `swissarmyhammer::{Result, SwissArmyHammerError}` 
- [ ] To `swissarmyhammer_common::{Result, SwissArmyHammerError}`
- [ ] Update all files using error types:
  - `src/mcp/error_handling.rs`
  - `src/mcp/file_watcher.rs`
  - `src/mcp/shared_utils.rs` 
  - `src/mcp/server.rs`
  - `src/mcp/tool_handlers.rs`
  - All other files with error imports
- [ ] Update error conversion logic

### Phase 6: Update Main Crate
- [ ] Keep main crate error types that are truly main-crate specific
- [ ] Re-export common errors for backward compatibility if needed
- [ ] Update main crate to depend on common crate for shared errors
- [ ] Remove duplicate error definitions

### Phase 7: Verification
- [ ] Build entire workspace to ensure no breakage
- [ ] Run all tests to verify error handling still works
- [ ] Verify domain crates can build independently 
- [ ] Check that error messages and types are preserved
- [ ] Ensure no circular dependencies exist

## Files to Update

### swissarmyhammer-common
- `src/error.rs` - Add core error types (may need to create)
- `src/lib.rs` - Export error types
- `Cargo.toml` - Ensure minimal dependencies

### swissarmyhammer-tools (Import Updates)
- `src/mcp/error_handling.rs` - Update error imports
- `src/mcp/file_watcher.rs` - Update error imports
- `src/mcp/shared_utils.rs` - Update error imports and conversions
- `src/mcp/server.rs` - Update error imports
- `src/mcp/tool_handlers.rs` - Update error imports
- All other files using `swissarmyhammer::{Result, SwissArmyHammerError}`

### Domain Crates
- Update any domain crates using main crate errors
- Add `swissarmyhammer-common` dependency
- Remove `swissarmyhammer` dependency for errors

### swissarmyhammer (Main Crate)
- `src/error.rs` - Move common errors out, keep main-specific errors
- `src/lib.rs` - Update exports
- `Cargo.toml` - Add dependency on common crate

## Success Criteria
- [ ] Core error types available in `swissarmyhammer-common`
- [ ] Domain crates don't depend on main crate for errors
- [ ] `swissarmyhammer-tools` uses common crate for errors
- [ ] All error handling functionality preserved
- [ ] No circular dependencies
- [ ] Workspace builds and tests pass
- [ ] Reduced coupling between components

## Risk Mitigation
- Start with copying errors before removing (ensure no breakage)
- Test error handling thoroughly after each phase
- Maintain backward compatibility during transition
- Keep error messages and behavior identical
- Plan rollback strategy for each phase

## Benefits
- **Independence**: Domain crates become truly independent
- **Reduced Coupling**: Eliminates major dependency on main crate
- **Consistency**: Shared error types across all components
- **Maintainability**: Central location for common error handling

## Notes
This is a foundational change that will enable other migration completions. Many incomplete migrations are blocked by this error dependency. Once complete, it will significantly reduce the coupling between swissarmyhammer-tools and the main crate.

Moving errors to `swissarmyhammer-common` follows the pattern of other shared utilities and makes sense architecturally since errors are truly cross-cutting concerns.