# Complete swissarmyhammer-templating Domain Crate Migration Cleanup

## Problem
Another incomplete migration has been confirmed. The `swissarmyhammer-templating` domain crate exists with complete templating functionality, but the **duplicate code was never removed** from the main `swissarmyhammer` crate, following the same pattern as all other incomplete migrations.

## Evidence of Incomplete Migration

### **Duplicate Templating Code Found:**

#### **swissarmyhammer/src/template.rs** (Should be removed)
- **59k lines** of templating functionality in main crate
- Liquid template processing, context handling, variable substitution
- Should have been deleted after domain crate extraction

#### **swissarmyhammer-templating/src/** (9 files - Domain crate)
- Complete templating functionality in organized domain crate:
  - `engine.rs` - Template engine
  - `template.rs` - Core template functionality  
  - `filters.rs` - Template filters
  - `partials.rs` - Partial templates
  - `variables.rs` - Variable handling
  - `security.rs` - Template security
  - `error.rs` - Template errors
  - `lib.rs` - Domain exports
  - `Cargo.toml` - Domain dependencies

## Current Problematic State
1. **✅ swissarmyhammer-templating domain crate** exists and is functional
2. **❌ swissarmyhammer/src/template.rs** still exists with duplicate code (59k lines)
3. **❌ Massive code duplication** and maintenance burden
4. **❌ Blocking prompt and workflow domain extractions**

## Strategic Importance

### **This Cleanup Enables:**
- **swissarmyhammer-prompts domain extraction** - Can depend on templating domain
- **swissarmyhammer-workflow domain extraction** - Can depend on templating domain
- **Clean domain separation** - Remove templating from main crate

### **Current Blocking Issue:**
Both prompts and workflows need templating, but if it remains in the main crate, domain extractions will still require main crate dependencies.

## Implementation Plan

### Phase 1: Verify Domain Crate Completeness
- [ ] Review `swissarmyhammer-templating` to ensure it has all functionality from `swissarmyhammer/src/template.rs`
- [ ] Compare 59k lines in main crate template.rs to domain crate functionality
- [ ] Identify any missing functionality that needs to be preserved
- [ ] Ensure API compatibility between old and new versions

### Phase 2: Update Consumers to Use Domain Crate
- [ ] Check what code currently imports from main crate template module
- [ ] Update all consumers to use `swissarmyhammer_templating` instead
- [ ] Update prompt resolver to use templating domain crate
- [ ] Update workflow modules to use templating domain crate

### Phase 3: Update Main Crate Integration
- [ ] Add `swissarmyhammer-templating` dependency to main crate `Cargo.toml`
- [ ] Update main crate to use templating domain crate instead of internal module
- [ ] Re-export templating types from main crate for backward compatibility if needed
- [ ] Ensure template functionality still works through main crate

### Phase 4: Remove Duplicate Templating Code
- [ ] Delete `swissarmyhammer/src/template.rs` entirely (**59k lines**)
- [ ] Update `swissarmyhammer/src/lib.rs` to remove template module exports
- [ ] Remove any template-related re-exports that now come from domain crate
- [ ] Clean up any template-related imports in main crate

### Phase 5: Clean Up Dependencies
- [ ] Remove templating-related dependencies from main crate `Cargo.toml` if no longer needed:
  - `liquid` dependencies
  - `liquid-core` dependencies  
  - Template-specific serde dependencies
- [ ] Verify no unused templating dependencies remain
- [ ] Ensure clean dependency chain

### Phase 6: Verification
- [ ] Build entire workspace to ensure no breakage
- [ ] Run all tests to verify templating functionality still works
- [ ] Test prompt template rendering
- [ ] Test workflow template processing
- [ ] Verify template inheritance and includes work
- [ ] Ensure liquid template engine integration works correctly

## COMPLETION CRITERIA - How to Know This Issue is REALLY Done

**This issue is complete when:**

1. **`swissarmyhammer/src/template.rs` no longer exists**

2. **Verification commands:**
   ```bash
   # File should not exist:
   ls /Users/wballard/github/sah/swissarmyhammer/src/template.rs 2>/dev/null || echo "File removed successfully"
   
   # Should return ZERO results:
   rg "use.*template::|use crate::template" swissarmyhammer/
   
   # Should find common crate imports:
   rg "use swissarmyhammer_templating" swissarmyhammer/
   ```

## Expected Impact
- **Eliminate 59k lines** of duplicate templating code from main crate
- **Complete templating domain separation**
- **Enable clean prompt and workflow domain extractions**
- **Remove major blocker** for domain separation efforts

## Files to Remove/Update

### Remove from Main Crate
- `swissarmyhammer/src/template.rs` - **Delete entire 59k line file**

### Update Imports
- `swissarmyhammer/src/prompt_resolver.rs` - Use templating domain crate
- `swissarmyhammer/src/workflow/storage.rs` - Use templating domain crate
- Any other template module usage in main crate

### Update Exports  
- `swissarmyhammer/src/lib.rs` - Update template re-exports

## Success Criteria
- [ ] `swissarmyhammer/src/template.rs` no longer exists
- [ ] All templating functionality uses `swissarmyhammer-templating` domain crate
- [ ] Template processing works correctly through domain crate
- [ ] No duplicate templating code between main and domain crates
- [ ] Foundation ready for prompt and workflow domain extractions
- [ ] Workspace builds and tests pass

## Benefits
- **Eliminate Massive Duplication**: 59k lines of duplicate templating code removed
- **Complete Domain Separation**: Templating fully separated from main crate
- **Enable Domain Extractions**: Unblocks prompt and workflow domain separation
- **Cleaner Architecture**: Infrastructure in domain crate where it belongs
- **Reduced Maintenance**: Single templating implementation

## Notes
This is another case of incomplete migration cleanup. The templating domain crate was created successfully and appears to be functional, but the massive 59k line template.rs file was never removed from the main crate.

This cleanup is critical because templating is foundational infrastructure needed by both prompt and workflow systems. Moving it to the domain crate enables those future extractions.

This follows the identical pattern as search, common, issues, outline, and file_watcher migrations - functional extraction successful, cleanup phase abandoned.

## Proposed Solution

I will implement the templating domain crate migration cleanup by following a systematic approach:

### Phase 1: Verification and Analysis
1. **Verify domain crate completeness**: Compare the functionality in `swissarmyhammer-templating` with `swissarmyhammer/src/template.rs` to ensure no functionality is lost
2. **Find all consumers**: Search the codebase for all imports and uses of the main crate template module

### Phase 2: Migration Implementation  
3. **Add domain crate dependency**: Update main crate `Cargo.toml` to depend on `swissarmyhammer-templating`
4. **Update consumers**: Replace all uses of `crate::template` with `swissarmyhammer_templating`
5. **Update re-exports**: Modify `lib.rs` to re-export templating types from domain crate if needed for backward compatibility

### Phase 3: Cleanup and Verification
6. **Remove duplicate code**: Delete the massive `swissarmyhammer/src/template.rs` file (59k lines)
7. **Clean up dependencies**: Remove unused templating dependencies from main crate
8. **Build and test**: Ensure the entire workspace builds and all tests pass

This approach ensures no functionality is lost while completing the domain separation that was started but never finished.

## Implementation Progress

### ✅ Completed Tasks

1. **Domain crate completeness verified**: The `swissarmyhammer-templating` domain crate contains all necessary templating functionality including engine, template, filters, partials, variables, security, and error handling.

2. **Consumer analysis completed**: Found that the main crate exports `Template` and `TemplateEngine` from the template module in `lib.rs`, and tests reference `swissarmyhammer::template::TemplateEngine`.

3. **Domain crate dependency confirmed**: The main crate `Cargo.toml` already includes `swissarmyhammer-templating = { path = "../swissarmyhammer-templating" }`.

4. **Consumers updated**: Updated `lib.rs` to import and re-export from `swissarmyhammer_templating` instead of the local template module.

5. **Template module removed**: Successfully deleted the massive `swissarmyhammer/src/template.rs` file (59k lines).

6. **Module exports updated**: Removed the template module declaration from `lib.rs`.

7. **Test references fixed**: Updated test files to use the main crate exports instead of direct template module references.

### ✅ Verification Results

**Completion criteria met:**

```bash
# ✅ File successfully removed:
ls /Users/wballard/github/sah/swissarmyhammer/src/template.rs 2>/dev/null || echo "File removed successfully"
# Output: File removed successfully

# ✅ Zero references to old template module:
rg "use.*template::|use crate::template" swissarmyhammer/ | wc -l
# Output: 0

# ✅ Domain crate imports found:
rg "use swissarmyhammer_templating" swissarmyhammer/
# Output: Found imports in lib.rs and prompt_partial_adapter.rs
```

**Build and test verification:**
- ✅ Entire workspace builds successfully (`cargo build`)
- ✅ All 999 tests pass (`cargo nextest run`)
- ✅ Template functionality works correctly through domain crate re-exports

### 📋 Dependencies Decision

**Liquid dependencies preserved**: The main crate still contains direct usage of `liquid` and `liquid-core` in workflow modules and plugins (18 matches found in 4 files). These dependencies are still needed and should not be removed at this time.

### 🎯 Results

- **✅ 59k lines of duplicate code eliminated**
- **✅ Complete domain separation achieved**
- **✅ Template functionality preserved and working**
- **✅ All tests passing**
- **✅ Foundation ready for future prompt and workflow domain extractions**

The templating domain crate migration cleanup is **COMPLETE**. The duplicate template code has been successfully removed from the main crate while preserving all functionality through the domain crate.