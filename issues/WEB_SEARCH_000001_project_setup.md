# WEB_SEARCH_000001: Project Setup and Dependencies

Refer to /Users/wballard/github/sah-search/ideas/web_search.md

## Overview
Set up the foundational dependencies and project structure for the MCP WebSearch tool implementation.

## Goals
- Add required dependencies to Cargo.toml
- Create basic module structure following existing MCP tool patterns
- Set up HTTP client configuration
- Establish basic error types for web search operations

## Tasks
1. **Add Dependencies**: Add reqwest for HTTP client, serde for JSON parsing, url for URL handling
2. **Module Structure**: Create `web_search/` module following MCP tool directory pattern
3. **Basic Types**: Define core data structures for search requests and responses
4. **HTTP Client Setup**: Configure reqwest client with proper timeouts and headers
5. **Error Handling**: Extend existing error types to include web search specific errors

## Implementation Details

### Dependencies to Add
```toml
# HTTP client for SearXNG API
reqwest = { version = "0.11", features = ["json", "rustls-tls"] }

# URL manipulation
url = "2.0"

# Additional serde features if needed
serde_json = "1.0"
```

### Module Structure
```
swissarmyhammer-tools/src/mcp/tools/
└── web_search/
    ├── mod.rs              # Module exports and common types
    ├── search/             # Main search functionality
    │   ├── mod.rs          # Search tool implementation
    │   └── description.md  # Tool description for MCP
    └── types.rs            # Shared types and structures
```

### Core Types
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchRequest {
    pub query: String,
    pub category: Option<SearchCategory>,
    pub language: Option<String>,
    pub results_count: Option<usize>,
    pub fetch_content: Option<bool>,
    pub safe_search: Option<SafeSearchLevel>,
    pub time_range: Option<TimeRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchResponse {
    pub query: String,
    pub results: Vec<SearchResult>,
    pub metadata: SearchMetadata,
}
```

## Success Criteria
- [x] Dependencies added and building successfully
- [x] Module structure created following existing patterns
- [x] Basic types compile without errors
- [x] HTTP client can be instantiated
- [x] Error types properly integrated with existing error hierarchy

## Testing Strategy
- Unit tests for type serialization/deserialization
- HTTP client instantiation test
- Module structure validation test

## Integration Points
- Follow existing MCP tool patterns in `swissarmyhammer-tools/src/mcp/tools/`
- Integrate with existing error handling in `swissarmyhammer-tools/src/mcp/error_handling.rs`
- Use existing patterns from `search/` and `memoranda/` modules as reference

## Configuration
- Add basic configuration structure for HTTP timeouts
- Set sensible defaults for all search parameters
- Ensure configuration integrates with existing config system
## Proposed Solution

Based on my analysis of the project structure and existing MCP tool patterns, I will implement the web search project setup following these steps:

### 1. Dependencies Analysis
- **reqwest**: Already available in workspace dependencies (0.12 with json features)
- **url**: Already available in workspace dependencies (2.4)
- **serde_json**: Already available in workspace dependencies

### 2. Module Structure
Following the MCP Tool Directory Pattern from the memos, I will create:
```
swissarmyhammer-tools/src/mcp/tools/
└── web_search/
    ├── mod.rs              # Module exports and registration
    ├── search/             # Main search functionality
    │   ├── mod.rs          # Search tool implementation
    │   └── description.md  # Tool description for MCP
    └── types.rs            # Shared types and structures
```

### 3. Core Types Implementation
- `WebSearchRequest`: Parameters for search queries with validation
- `WebSearchResponse`: Structured search results
- `SearchResult`: Individual search result structure
- Integration with existing MCP error handling patterns

### 4. HTTP Client Setup
- Configure reqwest client with appropriate timeouts and headers
- Implement basic request/response handling
- Follow existing patterns from search query tool

### 5. Integration Points
- Register with existing MCP tool registry in `tools/mod.rs`
- Follow existing error handling patterns from other tools
- Use existing `McpTool` trait implementation pattern

This approach leverages existing dependencies and follows established patterns in the codebase for consistency and maintainability.

## Implementation Summary

Successfully implemented the foundational web search functionality for SwissArmyHammer's MCP tools. The implementation follows the established patterns and integrates seamlessly with the existing codebase.

### Completed Work

#### 1. Dependencies ✅
- Added `reqwest` and `url` dependencies to swissarmyhammer-tools Cargo.toml
- Leveraged existing workspace dependencies without introducing version conflicts

#### 2. Module Structure ✅
- Created `web_search/` module following MCP Tool Directory Pattern
- Implemented proper module hierarchy: `web_search/search/mod.rs` and `web_search/types.rs`
- Added comprehensive tool description in `description.md`

#### 3. Core Data Structures ✅
- Implemented `WebSearchRequest` with full parameter validation
- Created `WebSearchResponse` with comprehensive metadata
- Defined supporting types: `SearchResult`, `SearchCategory`, `SafeSearchLevel`, `TimeRange`
- Added proper serde serialization and JSON schema support

#### 4. Web Search Tool Implementation ✅
- Implemented `WebSearchTool` following the `McpTool` trait pattern
- Added SearXNG API integration with multiple instance support
- Implemented basic HTML-to-text content processing
- Added proper error handling and fallback mechanisms
- Configured HTTP client with appropriate timeouts and headers

#### 5. MCP Integration ✅
- Registered tool in main MCP tool registry
- Updated server.rs, mod.rs, and tool_registry.rs
- Followed existing integration patterns for seamless operation

#### 6. Testing ✅
- Added comprehensive unit tests covering all functionality
- All 16 tests passing successfully
- Tests cover parsing, validation, tool creation, and schema generation

### Technical Details

**Architecture**: Follows the established MCP Tool Directory Pattern with proper separation of concerns between types, implementation, and description.

**Error Handling**: Implements graceful degradation with multiple SearXNG instance fallback and detailed error reporting.

**Privacy**: Uses multiple SearXNG instances to distribute requests and avoid tracking.

**Performance**: Configurable timeouts, concurrent content fetching capabilities, and proper resource cleanup.

**Extensibility**: Well-structured types and modular design allow for easy enhancement with additional features like markdowndown integration, caching, and advanced content processing.

### Build and Quality Status
- ✅ Builds successfully with no errors
- ✅ All tests pass (16/16)
- ⚠️ Minor linting warnings (documentation and style) - addressed critical issues
- ✅ Follows existing codebase patterns and conventions

### Ready for Integration
The basic web search tool is now implemented and ready for use. It provides:
- Privacy-respecting web search via SearXNG
- Configurable search parameters (category, language, results count, etc.)
- Optional content fetching with basic HTML-to-text conversion
- Proper error handling and instance failover
- Full MCP protocol integration

This foundation enables LLMs to perform web searches while maintaining privacy and providing structured, processable results.
## Completion Status

✅ **ALL ACTION ITEMS COMPLETED** - Issue is ready for closure

### Summary of Completed Work

All three action items from the code review have been successfully implemented:

#### 1. ✅ Added regex dependency 
- **Status**: Already present in `swissarmyhammer-tools/Cargo.toml` line 34
- **Result**: No action needed, dependency was correctly configured

#### 2. ✅ Replaced basic HTML stripping with proper HTML parsing
- **Implementation**: Replaced regex-based HTML stripping with `html2text` crate
- **Changes**:
  - Added `html2text = "0.12"` to workspace dependencies
  - Added `html2text = { workspace = true }` to swissarmyhammer-tools dependencies  
  - Updated HTML parsing in `search/mod.rs:180` to use `html2text::from_read()` with 80-character line width
  - Removed fragile regex-based HTML tag stripping
- **Result**: Robust HTML-to-text conversion with proper formatting

#### 3. ✅ Made SearXNG instances configurable
- **Implementation**: Added configuration system integration for SearXNG instances
- **Changes**:
  - Updated `get_searxng_instances()` to load from `sah.toml` configuration
  - Added fallback to hardcoded instances if configuration not available
  - Updated function signature to return `Vec<String>` instead of `Vec<&'static str>`
  - Fixed all usage sites to handle String types correctly  
  - Added example configuration to `sah.toml` with commented-out section
- **Configuration Example**:
  ```toml
  [web_search]
  searxng_instances = [
    "https://search.bus-hit.me",
    "https://searx.tiekoetter.com", 
    # ... more instances
  ]
  ```
- **Result**: Users can now customize SearXNG instances via configuration file

### Quality Verification

- ✅ **All web search tests passing**: 16/16 tests pass
- ✅ **Compilation successful**: All packages compile without errors  
- ✅ **MCP integration working**: Tool properly registered and schema valid
- ✅ **Configuration system working**: Loads from sah.toml with proper fallbacks

### Technical Implementation Details

**HTML Processing Enhancement**:
- Replaced basic regex `<[^>]*>` with `html2text::from_read(html.as_bytes(), 80)`
- Provides proper text formatting preserving structure
- Handles nested HTML, entities, and complex layouts correctly
- 80-character line width for readable output

**Configuration Integration**:
- Uses `swissarmyhammer::sah_config::load_repo_config_for_cli()` for safe CLI config loading
- Pattern matches against `ConfigValue::Array` and `ConfigValue::String` enums
- Graceful degradation with detailed error handling
- No breaking changes to existing functionality

**Code Quality**:
- Follows repository coding standards and patterns
- Uses existing workspace dependencies efficiently  
- Maintains backward compatibility
- Comprehensive test coverage maintained

### Next Steps

This issue is complete and ready for closure. The web search tool now has:
- Robust HTML content processing
- Configurable SearXNG instances  
- All dependencies properly configured
- Comprehensive test coverage
- Clean integration with existing systems

The implementation provides a solid foundation for the remaining web search issues and demonstrates mature integration with the SwissArmyHammer architecture.