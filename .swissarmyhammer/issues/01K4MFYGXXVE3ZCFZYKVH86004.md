# Complete swissarmyhammer-search Domain Crate Migration

## Problem
The migration from `swissarmyhammer/src/search` to the `swissarmyhammer-search` domain crate was started but never completed. Currently both exist:

1. **swissarmyhammer-search** - Domain crate exists with complete functionality
2. **swissarmyhammer/src/search** - Old code still exists in main crate  
3. **swissarmyhammer-tools** - Still importing from main crate instead of domain crate

This creates duplication, confusion, and prevents the goal of reducing dependencies on the main crate.

## Current State
- ✅ `swissarmyhammer-search` crate exists and is functional
- ❌ `swissarmyhammer-tools` still uses `swissarmyhammer::search` imports
- ❌ `swissarmyhammer/src/search` directory still exists
- ❌ Migration incomplete

## Evidence of Incomplete Migration
swissarmyhammer-tools imports that need updating:
```rust
// Current (wrong):
use swissarmyhammer::search::{FileIndexer, SemanticConfig, VectorStorage};
use swissarmyhammer::search::{SearchQuery, SemanticConfig, SemanticSearcher, VectorStorage};

// Should be (correct):
use swissarmyhammer_search::{FileIndexer, SemanticConfig, VectorStorage};
use swissarmyhammer_search::{SearchQuery, SemanticConfig, SemanticSearcher, VectorStorage};
```

## Implementation Plan

### Phase 1: Update swissarmyhammer-tools Dependencies
- [ ] Add `swissarmyhammer-search` to `swissarmyhammer-tools/Cargo.toml`
- [ ] Update imports in `swissarmyhammer-tools/src/mcp/tools/search/index/mod.rs`
- [ ] Update imports in `swissarmyhammer-tools/src/mcp/tools/search/query/mod.rs`
- [ ] Verify all search-related functionality still works
- [ ] Run tests to ensure no regressions

### Phase 2: Remove Old Search Code
- [ ] Remove `swissarmyhammer/src/search/` directory entirely
- [ ] Update `swissarmyhammer/src/lib.rs` to remove search module exports
- [ ] Remove search-related dependencies from main `swissarmyhammer/Cargo.toml` if no longer needed
- [ ] Update any other references in the main crate

### Phase 3: Clean Up Dependencies
- [ ] Remove `swissarmyhammer-search` dependency from `swissarmyhammer/Cargo.toml` if it exists
- [ ] Verify that swissarmyhammer-tools can build independently with domain crate
- [ ] Check for any other components using the old search module

### Phase 4: Verification
- [ ] Build entire workspace to ensure no breakage
- [ ] Run all tests, especially search-related ones
- [ ] Verify semantic search functionality works end-to-end
- [ ] Check that file indexing and querying still work through MCP tools

## Files to Update

### swissarmyhammer-tools
- `Cargo.toml` - Add swissarmyhammer-search dependency
- `src/mcp/tools/search/index/mod.rs` - Update imports
- `src/mcp/tools/search/query/mod.rs` - Update imports

### swissarmyhammer (main crate)
- Remove `src/search/` directory entirely:
  - `src/search/mod.rs`
  - `src/search/embedding.rs`
  - `src/search/indexer.rs`
  - `src/search/parser.rs`
  - `src/search/searcher.rs`
  - `src/search/storage.rs`
  - `src/search/types.rs`
  - `src/search/utils.rs`
  - `src/search/tests.rs`
- `src/lib.rs` - Remove search module exports

## Success Criteria
- [ ] swissarmyhammer-tools uses swissarmyhammer-search domain crate
- [ ] swissarmyhammer/src/search/ no longer exists
- [ ] All search functionality continues to work
- [ ] No duplicate code between crates
- [ ] Workspace builds and tests pass
- [ ] Reduced coupling between swissarmyhammer-tools and main crate

## Risk Mitigation
- Test thoroughly after each phase
- Keep git commits granular for easy rollback
- Verify API compatibility between old and new search modules
- Check for any undocumented dependencies on the old search code

## Notes
This completes a partially-done migration and removes another major dependency area from swissarmyhammer-tools → swissarmyhammer, supporting the broader goal of domain separation.