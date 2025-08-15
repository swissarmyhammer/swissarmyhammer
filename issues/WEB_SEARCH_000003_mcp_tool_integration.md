# WEB_SEARCH_000003: MCP Tool Integration

Refer to /Users/wballard/github/sah-search/ideas/web_search.md

## Overview
Create the MCP tool interface for web search, enabling Claude and other MCP clients to perform web searches through the tool protocol.

## Goals
- Implement the `web_search` MCP tool handler
- Create tool description and parameter schema
- Integrate with existing MCP server infrastructure
- Support all search parameters defined in the specification
- Return properly formatted MCP responses

## Tasks
1. **Tool Handler**: Implement the main web_search tool handler function
2. **Parameter Schema**: Define JSON schema for tool parameters
3. **MCP Integration**: Register tool with existing MCP server
4. **Response Formatting**: Format search results for MCP response
5. **CLI Integration**: Add CLI command for testing the web search tool

## Implementation Details

### MCP Tool Handler
```rust
// swissarmyhammer-tools/src/mcp/tools/web_search/search/mod.rs
pub async fn handle_web_search(
    args: serde_json::Value,
) -> Result<CallToolResult, Box<dyn std::error::Error + Send + Sync>> {
    // Parse arguments from MCP request
    let request: WebSearchRequest = serde_json::from_value(args)?;
    
    // Validate parameters
    validate_search_request(&request)?;
    
    // Perform search with SearXNG client
    let client = SearXngClient::new("https://search.example.org", Duration::from_secs(30))?;
    let results = client.search(&request).await?;
    
    // Format response for MCP
    let response = format_mcp_response(&request, &results)?;
    Ok(response)
}
```

### Tool Registration
```rust
// Register in tool_registry.rs
pub fn register_web_search_tools(registry: &mut ToolRegistry) {
    registry.register(
        "web_search",
        "Perform web searches using SearXNG and fetch result content",
        handle_web_search,
    );
}
```

### Parameter Schema
Following the specification:
```json
{
  "type": "object",
  "properties": {
    "query": {
      "type": "string",
      "description": "The search query string",
      "minLength": 1,
      "maxLength": 500
    },
    "category": {
      "type": "string", 
      "enum": ["general", "images", "videos", "news", "map", "music", "it", "science", "files"],
      "default": "general"
    },
    "results_count": {
      "type": "integer",
      "minimum": 1,
      "maximum": 50, 
      "default": 10
    },
    "fetch_content": {
      "type": "boolean",
      "default": true
    }
  },
  "required": ["query"]
}
```

### MCP Response Format
```json
{
  "content": [{
    "type": "text",
    "text": "Found 10 search results for query 'rust async programming'"  
  }],
  "is_error": false,
  "metadata": {
    "query": "rust async programming",
    "results_count": 10,
    "search_time_ms": 1250,
    "results": [
      {
        "title": "Async Programming in Rust - The Rust Book",
        "url": "https://doc.rust-lang.org/book/ch16-00-concurrency.html", 
        "description": "Learn about asynchronous programming in Rust...",
        "score": 0.95,
        "engine": "duckduckgo"
      }
    ]
  }
}
```

### CLI Integration
Add web search command to CLI:
```rust
// swissarmyhammer-cli/src/cli.rs
#[derive(Debug, Clone, Parser)]
pub enum Commands {
    // ... existing commands ...
    WebSearch {
        #[command(subcommand)]
        subcommand: WebSearchCommands,
    },
}

#[derive(Debug, Clone, Parser)]
pub enum WebSearchCommands {
    Search {
        query: String,
        #[arg(long, default_value = "10")]
        results: usize,
        #[arg(long)]
        category: Option<String>,
        #[arg(long, default_value = "table")]
        format: OutputFormat,
    },
}
```

## Success Criteria
- [x] MCP tool registered and discoverable via MCP protocol
- [x] Tool parameters properly validated according to schema
- [x] Search requests successfully processed and return results
- [x] MCP responses properly formatted and structured
- [x] CLI command works and displays search results
- [x] Error handling provides clear, actionable error messages

## Testing Strategy
- MCP tool integration tests with mock SearXNG responses
- Parameter validation tests for edge cases
- CLI integration tests for web search commands
- Error handling tests for various failure scenarios
- Response format validation tests

## Integration Points
- Uses SearXNG client from WEB_SEARCH_000002
- Follows existing MCP tool patterns from issues/, memoranda/, search/ modules
- Integrates with CLI infrastructure similar to existing search commands
- Uses existing error handling and response formatting patterns

## Configuration
- Default SearXNG instance URL (should be configurable later)
- Default search parameters (results count, safe search, etc.)
- Timeout values for HTTP requests
- MCP response format settings

## Sample Usage
```bash
# Via CLI
sh web-search search "rust async programming" --results 5 --format json

# Via MCP protocol  
{
  "method": "call_tool",
  "params": {
    "name": "web_search",
    "arguments": {
      "query": "rust async programming",
      "results_count": 5,
      "category": "it"
    }
  }
}
```