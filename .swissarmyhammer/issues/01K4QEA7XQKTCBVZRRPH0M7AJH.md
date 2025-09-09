# Reorganize Root-Level Types Files to Tool-Specific Locations

## Problem
`swissarmyhammer-tools` has several types files at the root MCP level that should be co-located with their specific tools rather than being scattered in the root. This violates good organization principles where types should live with the functionality that uses them.

## Current Misorganized Types Files

### **Root-Level Types (Should be moved to tool directories):**

#### **src/mcp/memo_types.rs** 
- **Content**: `CreateMemoRequest` and other memoranda types
- **Should be**: `src/mcp/tools/memoranda/types.rs`
- **Reason**: Only used by memoranda tools

#### **src/mcp/search_types.rs**
- **Content**: `SearchIndexRequest`, `SearchQueryRequest` and search types  
- **Should be**: `src/mcp/tools/search/types.rs`
- **Reason**: Only used by search tools

#### **src/mcp/notify_types.rs**
- **Content**: `NotifyLevel`, notification request types
- **Should be**: `src/mcp/tools/notify/types.rs` 
- **Reason**: Only used by notify tools

### **Already Correctly Organized:**
- ✅ `src/mcp/tools/web_search/types.rs` - Correctly co-located with web search tool

### **Root-Level Types (Legitimately Shared):**
- ✅ `src/mcp/types.rs` - General MCP types used across multiple tools (correctly placed)

## Implementation Plan

### Phase 1: Move memo_types.rs to Memoranda Tool
- [ ] Create `src/mcp/tools/memoranda/types.rs` 
- [ ] Move content from `src/mcp/memo_types.rs` to `src/mcp/tools/memoranda/types.rs`
- [ ] Update `src/mcp/tools/memoranda/mod.rs` to export types: `pub mod types;`
- [ ] Update imports throughout memoranda tools:
  ```rust
  // FROM: use crate::mcp::memo_types::CreateMemoRequest;
  // TO:   use super::types::CreateMemoRequest;
  ```
- [ ] Delete `src/mcp/memo_types.rs`

### Phase 2: Move search_types.rs to Search Tool  
- [ ] Create `src/mcp/tools/search/types.rs`
- [ ] Move content from `src/mcp/search_types.rs` to `src/mcp/tools/search/types.rs`
- [ ] Update `src/mcp/tools/search/mod.rs` to export types: `pub mod types;`
- [ ] Update imports throughout search tools:
  ```rust
  // FROM: use crate::mcp::search_types::{SearchIndexRequest, SearchQueryRequest};
  // TO:   use super::types::{SearchIndexRequest, SearchQueryRequest};
  ```
- [ ] Delete `src/mcp/search_types.rs`

### Phase 3: Move notify_types.rs to Notify Tool
- [ ] Create `src/mcp/tools/notify/types.rs`
- [ ] Move content from `src/mcp/notify_types.rs` to `src/mcp/tools/notify/types.rs` 
- [ ] Update `src/mcp/tools/notify/mod.rs` to export types: `pub mod types;`
- [ ] Update imports throughout notify tools:
  ```rust
  // FROM: use crate::mcp::notify_types::NotifyLevel;
  // TO:   use super::types::NotifyLevel;
  ```
- [ ] Delete `src/mcp/notify_types.rs`

### Phase 4: Update Module Organization
- [ ] Ensure each tool directory has consistent structure:
  ```
  tools/
  ├── memoranda/
  │   ├── mod.rs
  │   ├── types.rs      ← Now co-located
  │   ├── create/
  │   └── ...
  ├── search/
  │   ├── mod.rs  
  │   ├── types.rs      ← Now co-located
  │   ├── index/
  │   └── query/
  ├── notify/
  │   ├── mod.rs
  │   ├── types.rs      ← Now co-located
  │   └── create/
  ```
- [ ] Update mod.rs files to properly export types from each tool

### Phase 5: Verification
- [ ] Build swissarmyhammer-tools to ensure no compilation errors
- [ ] Run tests to verify all tools still work correctly
- [ ] Test MCP tool registration and schema generation
- [ ] Verify type imports and exports work correctly

## COMPLETION CRITERIA - How to Know This Issue is REALLY Done

**This issue is complete when:**

```bash
# Should return ZERO results (root types moved to tools):
ls /Users/wballard/github/sah/swissarmyhammer-tools/src/mcp/memo_types.rs 2>/dev/null && echo "NOT DONE"
ls /Users/wballard/github/sah/swissarmyhammer-tools/src/mcp/search_types.rs 2>/dev/null && echo "NOT DONE" 
ls /Users/wballard/github/sah/swissarmyhammer-tools/src/mcp/notify_types.rs 2>/dev/null && echo "NOT DONE"

# Should find types in tool directories:
ls /Users/wballard/github/sah/swissarmyhammer-tools/src/mcp/tools/memoranda/types.rs
ls /Users/wballard/github/sah/swissarmyhammer-tools/src/mcp/tools/search/types.rs
ls /Users/wballard/github/sah/swissarmyhammer-tools/src/mcp/tools/notify/types.rs
```

## Expected File Structure After Reorganization

```
src/mcp/
├── types.rs                    ← Keep (shared MCP types)
├── tools/
│   ├── memoranda/
│   │   ├── types.rs           ← Moved from memo_types.rs
│   │   └── ...
│   ├── search/
│   │   ├── types.rs           ← Moved from search_types.rs  
│   │   └── ...
│   ├── notify/
│   │   ├── types.rs           ← Moved from notify_types.rs
│   │   └── ...
│   └── web_search/
│       ├── types.rs           ← Already correctly placed
│       └── ...
```

## Benefits
- **Better Organization**: Types co-located with the tools that use them
- **Clearer Dependencies**: Easy to see what types belong to what tools
- **Easier Maintenance**: Tool-specific types grouped with tool implementation  
- **Consistent Structure**: All tools follow same organization pattern
- **Reduced Root Clutter**: Fewer files at root MCP level

## Notes
This follows standard Rust module organization principles where types should be co-located with the functionality that uses them. Currently, tool-specific types are scattered at the root level, making it unclear which types belong to which tools.

Moving types to tool-specific locations makes the codebase more organized and easier to navigate.