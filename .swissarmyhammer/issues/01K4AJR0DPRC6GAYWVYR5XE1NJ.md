eliminate memoranda search as a feature end to end
eliminate memoranda search as a feature end to end

## Proposed Solution

After investigating the codebase, the memoranda search functionality exists in several layers:

### Components to Remove:
1. **MCP Tool**: `SearchMemoTool` in `swissarmyhammer-tools/src/mcp/tools/memoranda/search/mod.rs`
2. **Tool Description**: `swissarmyhammer-tools/src/mcp/tools/memoranda/search/description.md`
3. **Core Search Functionality**: 
   - `search_memos()` and `search_memos_advanced()` methods in `MemoStorage` trait
   - `SearchOptions` and `SearchResult` types in memoranda module
   - `AdvancedMemoSearchEngine` in `advanced_search.rs` 
   - Search-related helper functions in storage implementations
4. **Tool Registry Integration**: Remove search tool registration
5. **Documentation**: Remove search references from docs and examples
6. **Tests**: Remove all search-related tests
7. **Dependencies**: Remove Tantivy dependency used only for search

### Implementation Steps:
1. Remove the `SearchMemoTool` and its registration
2. Remove search methods from `MemoStorage` trait and implementations
3. Remove `SearchOptions`, `SearchResult` types and `AdvancedMemoSearchEngine`
4. Remove search-related helper functions and utilities
5. Remove search-related tests throughout the codebase
6. Update Cargo.toml to remove Tantivy dependency if only used for search
7. Update documentation and examples to remove search references
8. Clean up imports and unused code

This will completely eliminate the search feature while preserving all other memoranda functionality (create, read, update, delete, list).