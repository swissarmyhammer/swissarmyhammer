# Complete swissarmyhammer-search Domain Crate Migration Cleanup

## Problem
Another incomplete migration has been confirmed. The `swissarmyhammer-search` domain crate exists with complete search functionality, but the **duplicate code was never removed** from the main `swissarmyhammer` crate, following the same pattern as common, issues, and other incomplete migrations.

## Evidence of Incomplete Migration

### **Duplicate Search Code Found:**

#### **swissarmyhammer/src/search/** (9 files - Should be removed)
- `embedding.rs` - Embedding functionality
- `indexer.rs` - File indexing logic
- `mod.rs` - Search module organization  
- `parser.rs` - Code parsing for search
- `searcher.rs` - Search query execution
- `storage.rs` - Vector storage implementation
- `tests.rs` - Search integration tests
- `types.rs` - Search type definitions
- `utils.rs` - Search utilities

#### **swissarmyhammer-search/src/** (12 files - Domain crate)
- Complete search functionality in organized domain crate
- `embedding.rs`, `indexer.rs`, `operations.rs`, `searcher.rs`, etc.
- Equivalent/enhanced versions of main crate search code

## Current Problematic State
1. **✅ swissarmyhammer-search domain crate** exists and is functional
2. **❌ swissarmyhammer/src/search/** still exists with duplicate code (9 files)
3. **❌ Code duplication** and maintenance burden
4. **❌ Potential confusion** about which search system to use

## Implementation Plan

### Phase 1: Verify Domain Crate Completeness
- [ ] Review `swissarmyhammer-search` to ensure it has all functionality from `swissarmyhammer/src/search/`
- [ ] Compare each file in main crate search to equivalent in domain crate
- [ ] Identify any missing functionality that needs to be preserved
- [ ] Ensure API compatibility and feature parity

### Phase 2: Verify No Usage of Old Search Code
- [ ] Confirm no code imports from `swissarmyhammer::search`
- [ ] Verify all search functionality goes through `swissarmyhammer-search` domain crate
- [ ] Check that main crate doesn't internally use its own search module
- [ ] Ensure swissarmyhammer-tools already uses domain crate (it should)

### Phase 3: Remove Duplicate Search Code from Main Crate
- [ ] Remove `swissarmyhammer/src/search/` directory entirely:
  - `embedding.rs`
  - `indexer.rs` 
  - `mod.rs`
  - `parser.rs`
  - `searcher.rs`
  - `storage.rs`
  - `tests.rs`
  - `types.rs`
  - `utils.rs`
- [ ] Update `swissarmyhammer/src/lib.rs` to remove search module exports
- [ ] Remove any search-related re-exports from main crate

### Phase 4: Clean Up Dependencies
- [ ] Remove search-related dependencies from main crate `Cargo.toml` if no longer needed:
  - Vector database dependencies (duckdb, etc.)
  - Embedding dependencies (fastembed, ort, etc.)  
  - TreeSitter dependencies for search parsing
- [ ] Verify main crate doesn't have `swissarmyhammer-search` as circular dependency
- [ ] Clean up any unused search-related imports

### Phase 5: Update Main Crate Integration
- [ ] If main crate needs search functionality, add proper dependency on search domain crate
- [ ] Re-export search types from main crate for backward compatibility if needed
- [ ] Ensure clean separation between main crate and search domain

### Phase 6: Verification
- [ ] Build entire workspace to ensure no breakage
- [ ] Run all tests to verify search functionality still works
- [ ] Verify semantic search operations work through domain crate
- [ ] Test search indexing and querying through MCP tools
- [ ] Ensure no functionality is lost in the cleanup

## COMPLETION CRITERIA - How to Know This Issue is REALLY Done

**This issue is complete when:**

1. **`swissarmyhammer/src/search/` directory no longer exists**

2. **Verification commands:**
   ```bash
   # Should return ZERO results:
   rg "use swissarmyhammer::search" /Users/wballard/github/sah/
   
   # Directory should not exist:
   ls /Users/wballard/github/sah/swissarmyhammer/src/search 2>/dev/null || echo "Directory removed successfully"
   ```

## Expected Impact
- **Eliminate duplicate search code** in main crate (~9 files)
- **Reduce main crate size** significantly 
- **Complete search domain separation**
- **Prevent confusion** about which search system to use
- **Reduce maintenance burden** of duplicate code

## Files to Remove

### swissarmyhammer/src/search/ (Entire Directory)
- `embedding.rs` - ~1000+ lines of embedding logic
- `indexer.rs` - ~1000+ lines of indexing logic
- `mod.rs` - Module organization and exports
- `parser.rs` - ~2000+ lines of parsing logic
- `searcher.rs` - ~1000+ lines of search logic  
- `storage.rs` - ~1500+ lines of vector storage
- `tests.rs` - Search integration tests
- `types.rs` - Search type definitions
- `utils.rs` - Search utility functions

### swissarmyhammer Updates
- `src/lib.rs` - Remove search module exports
- `Cargo.toml` - Remove search-specific dependencies if unused

## Success Criteria
- [ ] `swissarmyhammer/src/search/` no longer exists
- [ ] All search functionality continues to work through domain crate
- [ ] No duplicate search code between main and domain crates
- [ ] No unused search dependencies in main crate
- [ ] Workspace builds and tests pass
- [ ] Search indexing and querying work correctly through MCP tools

## Risk Mitigation
- Verify domain crate has complete functionality before removal
- Test search operations thoroughly after cleanup
- Ensure semantic search, indexing, and querying work correctly
- Keep git commits granular for easy rollback
- Check for any hidden dependencies on main crate search code

## Benefits
- **Eliminate Duplication**: Single search implementation in domain crate
- **Complete Domain Separation**: Search fully separated from main crate
- **Reduced Maintenance**: No duplicate search code to maintain
- **Cleaner Architecture**: Clear boundaries between search and main functionality
- **Smaller Main Crate**: Significant reduction in main crate size

## Notes
This follows the exact pattern as issues and common migrations - the functional migration was completed successfully, but the cleanup phase was skipped. The search domain crate is complete and working, but the old code remains in the main crate creating duplication and confusion.

This cleanup is purely removing dead/duplicate code since the search domain crate already handles all search functionality.