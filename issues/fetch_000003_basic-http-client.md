# Implement Basic HTTP Client Functionality

## Overview
Implement core HTTP client functionality using markdowndown crate for web content fetching. This step focuses on basic HTTP GET requests with minimal configuration. Refer to /Users/wballard/github/sah-fetch/ideas/fetch.md.

## Tasks
- Research markdowndown crate API for HTTP client configuration
- Implement basic URL fetching functionality in `WebFetchTool::execute()`
- Add request types and response structures
- Configure HTTP client with basic timeout and user-agent
- Implement basic error handling for network failures

## Implementation Details
- Use markdowndown's built-in HTTP client capabilities
- Configure default timeout (30 seconds as per specification)
- Set default user-agent: "SwissArmyHammer-Bot/1.0"
- Handle basic HTTP status codes (200, 4xx, 5xx)
- Return raw HTML content initially (markdown conversion comes later)

## Success Criteria
- Can fetch content from HTTPS URLs successfully
- Basic error handling for connection failures and timeouts
- HTTP status codes are properly handled
- Tool returns appropriate MCP CallToolResult format
- Basic logging with tracing crate

## Dependencies
- Requires fetch_000002_mcp-tool-structure (for tool structure)
- Requires fetch_000001_markdowndown-dependency (for HTTP client)

## Estimated Impact
- Enables basic web content retrieval
- Foundation for HTML-to-markdown conversion