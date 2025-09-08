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