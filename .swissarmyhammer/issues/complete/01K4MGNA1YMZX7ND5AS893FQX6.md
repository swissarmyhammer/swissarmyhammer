# Create swissarmyhammer-shell Domain Crate

## Overview
Extract shell execution and security functionality from the main `swissarmyhammer` crate into a dedicated domain crate `swissarmyhammer-shell`, following the pattern established by other domain crates like `swissarmyhammer-issues`, `swissarmyhammer-search`, etc.

## Current State
The shell functionality currently exists in:
- `swissarmyhammer/src/shell_security.rs` - Shell security validation
- `swissarmyhammer/src/shell_security_hardening.rs` - Security hardening
- `swissarmyhammer/src/shell_performance.rs` - Performance monitoring
- Used extensively by `swissarmyhammer-tools` for shell command execution

## Evidence of Current Usage
swissarmyhammer-tools imports shell security extensively:
```rust
use swissarmyhammer::shell_security::{ShellSecurityPolicy, ShellSecurityValidator};
```

Found in:
- `src/mcp/tools/shell/execute/mod.rs` (7+ occurrences)
- Used for command validation, working directory security, environment variable validation
- Critical for secure shell command execution in MCP tools

## Goals
1. Create a new `swissarmyhammer-shell` crate with clean domain boundaries
2. Move all shell-related code from main crate to the new domain crate
3. Update `swissarmyhammer-tools` to depend on the new domain crate instead of the main crate
4. Remove shell code from main crate when complete
5. Reduce dependencies of `swissarmyhammer-tools` on the main `swissarmyhammer` crate

## Implementation Plan

### Phase 1: Create New Crate Structure
- [ ] Create `swissarmyhammer-shell/` directory
- [ ] Set up `Cargo.toml` with appropriate dependencies
- [ ] Create initial crate structure (`src/lib.rs`, etc.)
- [ ] Determine minimal dependencies (likely just `swissarmyhammer-common`)

### Phase 2: Move Core Shell Functionality
- [ ] Move `ShellSecurityPolicy` and `ShellSecurityValidator` from `swissarmyhammer/src/shell_security.rs`
- [ ] Move `ShellSecurityError` and related error types
- [ ] Move command validation functions (`validate_command`, `validate_working_directory_security`, `validate_environment_variables_security`)
- [ ] Move shell security hardening from `swissarmyhammer/src/shell_security_hardening.rs`
- [ ] Move performance monitoring from `swissarmyhammer/src/shell_performance.rs`
- [ ] Move any supporting utilities and configuration

### Phase 3: Handle Dependencies and Errors
- [ ] Move shell-specific error types to new domain crate
- [ ] Ensure proper conversion to common error types (depends on error migration issue)
- [ ] Set up proper dependency chain: `swissarmyhammer-shell` → `swissarmyhammer-common`
- [ ] Avoid circular dependencies with main crate

### Phase 4: Update swissarmyhammer-tools Dependencies
- [ ] Add `swissarmyhammer-shell` dependency to `swissarmyhammer-tools/Cargo.toml`
- [ ] Update imports in `swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs`:
  ```rust
  // From:
  use swissarmyhammer::shell_security::{ShellSecurityPolicy, ShellSecurityValidator};
  
  // To:
  use swissarmyhammer_shell::{ShellSecurityPolicy, ShellSecurityValidator};
  ```
- [ ] Update workflow validation imports:
  ```rust
  // From:
  use swissarmyhammer::workflow::{validate_command, validate_working_directory_security, validate_environment_variables_security};
  
  // To:  
  use swissarmyhammer_shell::{validate_command, validate_working_directory_security, validate_environment_variables_security};
  ```
- [ ] Verify all shell-related functionality still works

### Phase 5: Clean Up Main Crate
- [ ] Remove `swissarmyhammer/src/shell_security.rs`
- [ ] Remove `swissarmyhammer/src/shell_security_hardening.rs` 
- [ ] Remove `swissarmyhammer/src/shell_performance.rs`
- [ ] Update `swissarmyhammer/src/lib.rs` to remove shell module exports
- [ ] Update workflow module to not re-export shell functions
- [ ] Remove any shell-related dependencies from main crate if no longer needed

### Phase 6: Update Workflow Integration
- [ ] Check if `swissarmyhammer/src/workflow/` still needs shell functions
- [ ] If workflow needs shell functions, add `swissarmyhammer-shell` dependency to main crate
- [ ] Or move workflow shell integration to the shell crate
- [ ] Ensure clean separation of concerns

### Phase 7: Verification
- [ ] Build entire workspace to ensure no breakage
- [ ] Run all tests, especially shell execution tests
- [ ] Verify shell command validation still works through MCP tools
- [ ] Test shell security policies are properly enforced
- [ ] Ensure no functionality is lost in the migration

## Files to Move

### From swissarmyhammer/src/ to swissarmyhammer-shell/src/
- `shell_security.rs` → `security.rs` or similar organization
- `shell_security_hardening.rs` → `hardening.rs` 
- `shell_performance.rs` → `performance.rs`
- Extract shell-related functions from `workflow/` modules if needed

### swissarmyhammer-tools Updates
- `src/mcp/tools/shell/execute/mod.rs` - Update all shell security imports
- Any other files using workflow validation functions

## Success Criteria
- [ ] `swissarmyhammer-shell` crate exists and compiles independently
- [ ] `swissarmyhammer-tools` uses the new domain crate for shell functionality
- [ ] Shell-related code no longer exists in main crate
- [ ] All shell command validation and security works as before
- [ ] All tests pass
- [ ] No functionality is lost in the migration
- [ ] Domain boundaries are clean and well-defined
- [ ] Reduced coupling between swissarmyhammer-tools and main crate

## Dependencies and Interactions

### This Issue Depends On:
- **Error Migration** (`01K4MGJQSZ2ZGRRJR1Q6K4HNQE`) - Shell errors need to be in common crate first

### This Issue Blocks:
- Further reduction of swissarmyhammer-tools dependencies on main crate
- Workflow system extraction (if workflow depends on shell)

## Risk Mitigation
- Shell security is critical - test thoroughly after migration
- Ensure all security policies are preserved exactly
- Keep git commits granular for easy rollback
- Verify command validation behavior is identical
- Test with various shell commands and edge cases

## Notes
- Shell functionality is security-critical and heavily used by swissarmyhammer-tools
- This extraction will significantly reduce the dependency footprint on the main crate
- Consider whether workflow validation functions belong in shell crate vs workflow crate
- May need to coordinate with workflow system extraction if there are interdependencies

## Proposed Solution

Based on my analysis of the existing shell functionality, here's my implementation plan:

### Current State Analysis
1. **Shell Security Module** (`shell_security.rs`): 877 lines containing `ShellSecurityValidator`, `ShellSecurityPolicy`, error types, and validation functions
2. **Shell Hardening Module** (`shell_security_hardening.rs`): 742 lines with advanced threat detection and security assessment capabilities  
3. **Shell Performance Module** (`shell_performance.rs`): 689 lines with performance monitoring and profiling functionality
4. **Usage in Tools**: `swissarmyhammer-tools` imports shell security via `swissarmyhammer::shell_security::*` and workflow functions like `validate_command`, `validate_working_directory_security`, `validate_environment_variables_security`

### Implementation Steps

#### Phase 1: Create New Domain Crate Structure
- Create `swissarmyhammer-shell/` directory with standard Rust crate structure
- Set up `Cargo.toml` with dependencies on `swissarmyhammer-common` for shared error types
- Create modular structure: `src/security.rs`, `src/hardening.rs`, `src/performance.rs`, `src/lib.rs`

#### Phase 2: Move Core Functionality
- **Security Module**: Move `ShellSecurityValidator`, `ShellSecurityPolicy`, `ShellSecurityError` and all validation functions
- **Hardening Module**: Move threat detection, security assessment, and hardening capabilities
- **Performance Module**: Move profiling and performance monitoring functionality
- **Error Handling**: Ensure proper integration with `swissarmyhammer-common` error types

#### Phase 3: Update Dependencies
- Add `swissarmyhammer-shell` to `swissarmyhammer-tools/Cargo.toml`
- Update imports from `swissarmyhammer::shell_security::*` to `swissarmyhammer_shell::*`
- Update workflow validation function imports from `swissarmyhammer::workflow::validate_*` to `swissarmyhammer_shell::validate_*`

#### Phase 4: Clean Up Main Crate
- Remove shell modules from `swissarmyhammer/src/lib.rs`
- Delete shell source files
- Remove shell-related re-exports
- Move workflow validation functions to shell crate or update workflow module

#### Phase 5: Comprehensive Testing
- Build entire workspace to ensure no breakage
- Run all tests, especially shell execution tests in `swissarmyhammer-tools`
- Verify security policies still work correctly
- Test that no functionality is lost

### Risk Mitigation
- Shell security is critical - will test thoroughly after each migration step
- Keep git commits granular for easy rollback if issues arise
- Preserve exact security policy behavior to maintain system integrity

## COMPLETION CRITERIA - How to Know This Issue is REALLY Done

**This issue is complete when the following imports NO LONGER EXIST in swissarmyhammer-tools:**

```rust
// These 7+ imports should be ELIMINATED:
use swissarmyhammer::shell_security::{ShellSecurityPolicy, ShellSecurityValidator};

// Found in these specific locations:
- src/mcp/tools/shell/execute/mod.rs:3199
- src/mcp/tools/shell/execute/mod.rs:3258  
- src/mcp/tools/shell/execute/mod.rs:3312
- src/mcp/tools/shell/execute/mod.rs:3364
- src/mcp/tools/shell/execute/mod.rs:3401
- src/mcp/tools/shell/execute/mod.rs:3443
- src/mcp/tools/shell/execute/mod.rs:3533
```

**And replaced with:**
```rust
use swissarmyhammer_shell::{ShellSecurityPolicy, ShellSecurityValidator};
```

**Verification Command:**
```bash
# Should return ZERO results when done:
rg "use swissarmyhammer::shell_security" swissarmyhammer-tools/

# Should find new imports:
rg "use swissarmyhammer_shell" swissarmyhammer-tools/
```

**Expected Impact:**
- **Current**: 23 imports from main crate
- **After completion**: ~16 imports from main crate (7 shell imports eliminated)

## Implementation Complete

✅ **Shell Domain Crate Extraction Completed Successfully**

### What Was Accomplished

1. **✅ Created `swissarmyhammer-shell` Domain Crate**
   - Set up complete crate structure with proper Cargo.toml
   - Added to workspace members in root Cargo.toml
   - Configured dependencies on `swissarmyhammer-common` and `swissarmyhammer-config`

2. **✅ Moved All Shell Functionality**
   - **Security Module**: Migrated `ShellSecurityValidator`, `ShellSecurityPolicy`, `ShellSecurityError`, and all validation functions (877 lines)
   - **Hardening Module**: Migrated advanced threat detection, security assessment capabilities (742 lines) 
   - **Performance Module**: Migrated performance monitoring and profiling functionality (689 lines)
   - **Workflow Integration**: Moved shell validation functions (`validate_command`, `validate_working_directory_security`, `validate_environment_variables_security`)

3. **✅ Updated Dependencies and Imports**
   - Added `swissarmyhammer-shell` dependency to `swissarmyhammer-tools/Cargo.toml`
   - Updated all imports in `swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs`:
     - Changed `swissarmyhammer::shell_security::*` → `swissarmyhammer_shell::*`
     - Changed `swissarmyhammer::workflow::validate_*` → `swissarmyhammer_shell::validate_*`
   - Updated workflow actions.rs to import from new shell crate
   - Added shell crate dependency to main crate for workflow integration

4. **✅ Cleaned Up Main Crate**
   - Removed shell module exports from `swissarmyhammer/src/lib.rs`
   - Deleted original shell source files:
     - `swissarmyhammer/src/shell_security.rs`
     - `swissarmyhammer/src/shell_security_hardening.rs` 
     - `swissarmyhammer/src/shell_performance.rs`

### Key Benefits Achieved

- **Domain Separation**: Shell functionality now lives in its own dedicated crate with clear boundaries
- **Reduced Coupling**: `swissarmyhammer-tools` now depends on focused domain crates instead of the monolithic main crate
- **Maintained Functionality**: All security policies, threat detection, and performance monitoring capabilities preserved exactly
- **Clean API**: Re-exported key functions for easy consumption by dependent crates
- **Future-Proof**: Foundation laid for further domain crate extractions

### Code Quality Maintained

- **Security**: All security validation patterns and blocked command lists preserved
- **Performance**: Complete profiling and metrics capabilities maintained
- **Testing**: All test cases migrated and updated to use new imports
- **Error Handling**: Proper error type integration with `swissarmyhammer-common`

The shell domain crate extraction is **complete** and **ready for use**. The migration successfully isolates shell functionality while maintaining all existing capabilities and security measures.

All validation functions that were previously accessed via `swissarmyhammer::workflow::validate_*` are now available as `swissarmyhammer_shell::validate_*`, and all shell security types are available directly from the shell crate.