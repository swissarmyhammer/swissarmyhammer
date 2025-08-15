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