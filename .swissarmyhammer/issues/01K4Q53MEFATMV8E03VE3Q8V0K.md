# Complete swissarmyhammer-outline Domain Crate Migration Cleanup

## Problem
Another incomplete migration has been confirmed. The `swissarmyhammer-outline` domain crate exists with complete outline functionality, but the **duplicate code was never removed** from the main `swissarmyhammer` crate, following the same pattern as search, common, and issues incomplete migrations.

## Evidence of Incomplete Migration

### **Duplicate Outline Code Found:**

#### **swissarmyhammer/src/outline/** (16 files - Should be removed)
- `file_discovery.rs` - File discovery logic
- `formatter.rs` - Output formatting
- `hierarchy.rs` - Hierarchy building
- `integration_tests.rs` - Integration tests
- `mod.rs` - Module organization
- `parser.rs` - Code parsing logic
- `signature.rs` - Signature extraction
- `signature_integration_test.rs` - Signature tests
- `types.rs` - Type definitions
- `utils.rs` - Utility functions
- `extractors/mod.rs` - Language extractors module
- `extractors/dart.rs` - Dart code extraction
- `extractors/javascript.rs` - JavaScript code extraction
- `extractors/python.rs` - Python code extraction
- `extractors/rust.rs` - Rust code extraction
- `extractors/typescript.rs` - TypeScript code extraction

#### **swissarmyhammer-outline/src/** (15 files - Domain crate)
- Complete outline functionality in organized domain crate
- Equivalent/enhanced versions of main crate outline code
- Proper domain structure and organization

## Current Problematic State
1. **✅ swissarmyhammer-outline domain crate** exists and is functional
2. **❌ swissarmyhammer/src/outline/** still exists with duplicate code (16 files)
3. **❌ swissarmyhammer-tools still imports from main crate**:
   ```rust
   use swissarmyhammer::outline::types::OutlineNodeType;
   ```
4. **❌ Code duplication** and maintenance burden

## Implementation Plan

### Phase 1: Verify Domain Crate Completeness
- [ ] Review `swissarmyhammer-outline` to ensure it has all functionality from `swissarmyhammer/src/outline/`
- [ ] Compare each file in main crate outline to equivalent in domain crate
- [ ] Identify any missing functionality that needs to be preserved
- [ ] Ensure API compatibility and feature parity

### Phase 2: Update swissarmyhammer-tools to Use Domain Crate
- [ ] Add `swissarmyhammer-outline` dependency to `swissarmyhammer-tools/Cargo.toml` (if not already present)
- [ ] Update import in `swissarmyhammer-tools/src/mcp/tools/outline/generate/mod.rs:333`:
   ```rust
   // FROM: use swissarmyhammer::outline::types::OutlineNodeType;
   // TO:   use swissarmyhammer_outline::OutlineNodeType;
   ```
- [ ] Verify outline generation tool still works with domain crate
- [ ] Test outline functionality through MCP tools

### Phase 3: Verify No Other Usage of Old Outline Code
- [ ] Confirm no other code imports from `swissarmyhammer::outline`
- [ ] Verify main crate doesn't internally use its own outline module
- [ ] Check that all outline functionality goes through domain crate
- [ ] Ensure no hidden dependencies on main crate outline code

### Phase 4: Remove Duplicate Outline Code from Main Crate
- [ ] Remove `swissarmyhammer/src/outline/` directory entirely:
  - `file_discovery.rs`
  - `formatter.rs`
  - `hierarchy.rs`
  - `integration_tests.rs`
  - `mod.rs`
  - `parser.rs`
  - `signature.rs`
  - `signature_integration_test.rs`
  - `types.rs`
  - `utils.rs`
  - `extractors/` (entire subdirectory with 6 files)
- [ ] Update `swissarmyhammer/src/lib.rs` to remove outline module exports
- [ ] Remove any outline-related re-exports from main crate

### Phase 5: Clean Up Dependencies
- [ ] Remove outline-related dependencies from main crate `Cargo.toml` if no longer needed:
  - TreeSitter dependencies for outline parsing
  - Any outline-specific parsing dependencies
- [ ] Verify main crate doesn't have `swissarmyhammer-outline` as circular dependency
- [ ] Clean up any unused outline-related imports

### Phase 6: Verification
- [ ] Build entire workspace to ensure no breakage
- [ ] Run all tests to verify outline functionality still works
- [ ] Test outline generation through MCP tools
- [ ] Verify code structure analysis works correctly
- [ ] Ensure no functionality is lost in the cleanup

## COMPLETION CRITERIA - How to Know This Issue is REALLY Done

**This issue is complete when:**

1. **`swissarmyhammer/src/outline/` directory no longer exists**

2. **swissarmyhammer-tools import is updated:**
   ```bash
   # Should return ZERO results:
   rg "use swissarmyhammer::outline" swissarmyhammer-tools/
   
   # Should find new import:
   rg "use swissarmyhammer_outline" swissarmyhammer-tools/
   
   # Directory should not exist:
   ls /Users/wballard/github/sah/swissarmyhammer/src/outline 2>/dev/null || echo "Directory removed successfully"
   ```

## Expected Impact
- **Eliminate duplicate outline code** in main crate (~16 files)
- **Complete outline domain separation** 
- **Fix remaining outline dependency** in swissarmyhammer-tools
- **Reduce main crate size** significantly
- **Update dependency count**: 9 → 8 imports (1 outline import eliminated)

## Files to Remove

### swissarmyhammer/src/outline/ (Entire Directory)
- Core outline files: `mod.rs`, `types.rs`, `parser.rs`, `hierarchy.rs`, etc.
- Language extractors: `extractors/rust.rs`, `extractors/python.rs`, etc.
- Supporting files: `utils.rs`, `formatter.rs`, `signature.rs`, etc.
- Test files: `integration_tests.rs`, `signature_integration_test.rs`

### swissarmyhammer-tools Update
- `src/mcp/tools/outline/generate/mod.rs` - Update import to use domain crate

## Success Criteria
- [ ] `swissarmyhammer/src/outline/` no longer exists
- [ ] swissarmyhammer-tools uses `swissarmyhammer_outline` domain crate
- [ ] All outline functionality continues to work through domain crate
- [ ] No duplicate outline code between main and domain crates
- [ ] Workspace builds and tests pass
- [ ] Outline generation works correctly through MCP tools

## Notes
This is the 4th confirmed case of incomplete migration cleanup. The pattern is consistent: domain crates were created successfully and work correctly, but the old duplicate code was never removed from the main crate.

This cleanup will eliminate ~16 more duplicate files from the main crate and fix the remaining outline dependency in swissarmyhammer-tools, bringing us closer to full domain separation.