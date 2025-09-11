# Merge swissarmyhammer-issues-config into swissarmyhammer-issues

## Problem
The `swissarmyhammer-issues-config` crate is unnecessarily separate from `swissarmyhammer-issues`. Configuration for a domain should be part of the domain crate itself, not a separate crate. This creates unnecessary complexity and dependency management overhead.

## Current State Analysis

### **Two Separate Crates:**
- `swissarmyhammer-issues` - Main issues domain functionality
- `swissarmyhammer-issues-config` - Just configuration for issues

### **Unnecessary Separation:**
- Configuration is tightly coupled to the domain it configures
- No other domain has separate config crates
- Creates extra dependency management
- Violates principle of cohesive domain boundaries

## Evidence of Usage
Looking at the codebase:
- `swissarmyhammer-issues` depends on `swissarmyhammer-issues-config`
- `swissarmyhammer-tools` depends on both crates
- Configuration is only used by the issues domain

## Proposed Solution
**Merge `swissarmyhammer-issues-config` into `swissarmyhammer-issues`** to create a single cohesive domain crate.

## Implementation Plan

### Phase 1: Move Configuration Code
- [ ] Move all code from `swissarmyhammer-issues-config/src/` to `swissarmyhammer-issues/src/config/`
- [ ] Add `pub mod config;` to `swissarmyhammer-issues/src/lib.rs`
- [ ] Re-export config types: `pub use config::*;` for backward compatibility
- [ ] Preserve all configuration functionality

### Phase 2: Update Dependencies in swissarmyhammer-issues
- [ ] Remove `swissarmyhammer-issues-config` dependency from `swissarmyhammer-issues/Cargo.toml`
- [ ] Add any dependencies from issues-config crate directly to issues crate
- [ ] Update internal imports from external crate to internal module
- [ ] Ensure all config functionality works within issues crate

### Phase 3: Update Consumers
- [ ] Update `swissarmyhammer-tools/Cargo.toml`:
  - Remove `swissarmyhammer-issues-config` dependency
  - Keep only `swissarmyhammer-issues` dependency
- [ ] Update imports in swissarmyhammer-tools:
  ```rust
  // FROM: use swissarmyhammer_issues_config::Config;
  // TO:   use swissarmyhammer_issues::Config;
  ```
- [ ] Update any other consumers using the issues-config crate

### Phase 4: Update Main Crate
- [ ] Remove `swissarmyhammer-issues-config` dependency from `swissarmyhammer/Cargo.toml`
- [ ] Update any imports in main crate to use unified issues crate
- [ ] Verify issues functionality still works

### Phase 5: Remove Separate Config Crate
- [ ] Delete `swissarmyhammer-issues-config/` directory entirely
- [ ] Remove from workspace `Cargo.toml` members
- [ ] Update any documentation referring to separate config crate
- [ ] Clean up any references in CI/build scripts

### Phase 6: Verification
- [ ] Build entire workspace to ensure no breakage
- [ ] Run all tests to verify issues configuration still works
- [ ] Test issue creation, validation, and management
- [ ] Ensure configuration loading and validation works
- [ ] Verify no functionality is lost

## COMPLETION CRITERIA - How to Know This Issue is REALLY Done

**This issue is complete when:**

1. **`swissarmyhammer-issues-config/` directory no longer exists**
2. **Single dependency in consumers:**
   ```bash
   # Should find only issues dependency:
   rg "swissarmyhammer-issues" swissarmyhammer-tools/Cargo.toml
   
   # Should return ZERO results:
   rg "swissarmyhammer-issues-config" swissarmyhammer-tools/Cargo.toml
   
   # Directory should not exist:
   ls /Users/wballard/github/sah/swissarmyhammer-issues-config 2>/dev/null || echo "Directory removed successfully"
   ```

## Expected Benefits

### **Simpler Dependency Management:**
- **Before**: 2 separate crates (issues + issues-config)
- **After**: 1 unified crate (issues with internal config module)

### **Better Domain Cohesion:**
- Configuration belongs with the domain it configures
- Single source of truth for all issues-related functionality
- Easier maintenance and development

### **Consistency:**
- Other domains don't have separate config crates
- Follows standard pattern of config as internal module
- Cleaner workspace organization

## Files to Move/Update

### Move to swissarmyhammer-issues
- `swissarmyhammer-issues-config/src/*` → `swissarmyhammer-issues/src/config/`

### Update Consumers
- `swissarmyhammer-tools/Cargo.toml` - Remove issues-config dependency
- `swissarmyhammer-tools/src/mcp/tools/issues/*/mod.rs` - Update config imports
- `swissarmyhammer/Cargo.toml` - Remove issues-config dependency

### Remove
- `swissarmyhammer-issues-config/` - Entire directory
- Workspace member reference

## Success Criteria
- [ ] `swissarmyhammer-issues-config` crate no longer exists
- [ ] All configuration functionality available in `swissarmyhammer-issues`
- [ ] Consumers depend only on unified issues crate
- [ ] All issues functionality preserved and working
- [ ] Simpler dependency graph
- [ ] Workspace builds and tests pass

## Risk Mitigation
- Preserve all configuration functionality during merge
- Test issues configuration thoroughly after merge
- Keep git commits granular for easy rollback
- Verify all config validation and loading works
- Ensure no breaking changes for consumers

## Notes
This follows the principle of **cohesive domain boundaries**. Configuration for a domain should be part of that domain, not a separate crate. This simplifies the dependency graph and makes the issues domain more self-contained.

Other domains (search, workflow, etc.) manage their configuration internally rather than having separate config crates. Issues should follow the same pattern for consistency and simplicity.