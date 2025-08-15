# MCP WebFetch Tool Specification

## Overview

This specification defines a new MCP tool `web_fetch` that enables LLMs to retrieve and process web content. The tool fetches web pages, converts HTML to markdown, and provides structured content for analysis and processing in AI workflows.

## Problem Statement

LLMs often need to access web content for:
1. Research and information gathering
2. Documentation analysis and summarization
3. API documentation processing
4. Content validation and fact-checking
5. Real-time data retrieval from web sources
6. Processing of online resources referenced in code or documentation

Currently, there's no standardized way for MCP tools to fetch and process web content with proper HTML-to-markdown conversion and error handling.

## Solution: MCP WebFetch Tool

### Tool Definition

**Tool Name**: `web_fetch`  
**Purpose**: Fetch web content and convert HTML to markdown for AI processing  
**Usage Context**: Available to LLMs during MCP workflow execution and analysis tasks

### Parameters

```json
{
  "type": "object",
  "properties": {
    "url": {
      "type": "string",
      "format": "uri",
      "description": "The URL to fetch content from (must be a valid HTTP/HTTPS URL)"
    },
    "timeout": {
      "type": "integer",
      "description": "Request timeout in seconds (optional, defaults to 30 seconds)",
      "minimum": 5,
      "maximum": 120,
      "default": 30
    },
    "follow_redirects": {
      "type": "boolean",
      "description": "Whether to follow HTTP redirects (optional, defaults to true)",
      "default": true
    },
    "max_content_length": {
      "type": "integer",
      "description": "Maximum content length in bytes (optional, defaults to 1MB)",
      "minimum": 1024,
      "maximum": 10485760,
      "default": 1048576
    },
    "user_agent": {
      "type": "string",
      "description": "Custom User-Agent header (optional, defaults to SwissArmyHammer bot)",
      "default": "SwissArmyHammer-Bot/1.0"
    }
  },
  "required": ["url"]
}
```

### Implementation Requirements

#### Web Content Fetching
- Use `markdowndown` crate for HTML fetching and markdown conversion
- Support HTTP/HTTPS protocols with proper certificate validation
- Handle common HTTP response codes (200, 301, 302, 404, 500, etc.)
- Implement configurable timeout with reasonable defaults
- Support custom headers including User-Agent

#### Content Processing
- Convert HTML to clean, structured markdown using `markdowndown`
- Preserve important structural elements (headers, lists, links, code blocks)
- Remove unnecessary HTML elements (scripts, styles, ads)
- Handle different character encodings properly
- Extract and preserve metadata (title, description)

#### Error Handling
- Network connectivity issues
- Invalid URLs and malformed responses
- Timeout handling with partial content recovery
- Content size limits and truncation
- SSL/TLS certificate validation errors

#### Security Controls
- URL validation and sanitization
- Content-Type verification
- Protection against malicious redirects
- Rate limiting to prevent abuse
- Blacklist support for restricted domains

## Response Format

### Successful Fetch
```json
{
  "content": [{
    "type": "text",
    "text": "Successfully fetched content from URL"
  }],
  "is_error": false,
  "metadata": {
    "url": "https://example.com/page",
    "final_url": "https://example.com/page",
    "title": "Example Page Title",
    "content_type": "text/html",
    "content_length": 15420,
    "status_code": 200,
    "response_time_ms": 245,
    "markdown_content": "# Example Page Title\n\nThis is the converted markdown content...",
    "word_count": 856,
    "headers": {
      "server": "nginx/1.18.0",
      "content-encoding": "gzip"
    }
  }
}
```

### Redirect Response
```json
{
  "content": [{
    "type": "text",
    "text": "URL redirected to final destination"
  }],
  "is_error": false,
  "metadata": {
    "url": "https://example.com/old-page",
    "final_url": "https://example.com/new-page", 
    "redirect_count": 2,
    "status_code": 200,
    "markdown_content": "# Redirected Page Content...",
    "redirect_chain": [
      "https://example.com/old-page -> 301",
      "https://example.com/temp-page -> 302", 
      "https://example.com/new-page -> 200"
    ]
  }
}
```

### Error Response
```json
{
  "content": [{
    "type": "text",
    "text": "Failed to fetch content: Connection timeout"
  }],
  "is_error": true,
  "metadata": {
    "url": "https://example.com/page",
    "error_type": "timeout",
    "error_details": "Request timed out after 30 seconds",
    "status_code": null,
    "response_time_ms": 30000
  }
}
```

## Use Cases

### Documentation Research
```json
{
  "url": "https://docs.rust-lang.org/book/ch04-01-what-is-ownership.html",
  "timeout": 45,
  "max_content_length": 2097152
}
```

### API Documentation Processing
```json
{
  "url": "https://api.github.com/docs/rest/repos",
  "user_agent": "SwissArmyHammer-DocProcessor/1.0"
}
```

### Content Validation
```json
{
  "url": "https://example.com/changelog",
  "follow_redirects": true,
  "timeout": 15
}
```

### News and Content Analysis
```json
{
  "url": "https://blog.rust-lang.org/2024/01/15/recent-updates.html",
  "max_content_length": 5242880
}
```

## Integration with MarkdownDown

### Core Library Usage
The tool leverages the `markdowndown` crate for:
- HTML fetching with proper HTTP client configuration
- Content-Type detection and character encoding handling  
- HTML-to-markdown conversion with customizable options
- Built-in security features and URL validation

### Conversion Options
```rust
use markdowndown::{MarkdownOptions, fetch_and_convert};

let options = MarkdownOptions {
    preserve_code_blocks: true,
    convert_tables: true,
    preserve_links: true,
    remove_images: false,
    max_heading_level: 6,
};
```

### Error Mapping
Map `markdowndown` errors to MCP-appropriate error responses:
- Network errors → Connection/timeout errors
- Parse errors → Content processing errors  
- Security errors → Access denied errors
- Size limits → Content too large errors

## Security Considerations

### URL Validation
- Verify URL scheme (HTTP/HTTPS only)
- Check against domain blacklists and security policies
- Prevent SSRF (Server-Side Request Forgery) attacks

### Rate Limiting
- Circuit breaker pattern for problematic domains

### Privacy Protection
- Configurable User-Agent identification
- Optional request logging with privacy controls

## Configuration Options

### Global Settings

These are the defaults for parameters -- do not create a config file.
```toml
[web_fetch]
default_timeout = 30          # seconds
max_content_size = "1MB"      # maximum content size
max_redirects = 10            # maximum redirect hops
user_agent = "SwissArmyHammer-Bot/1.0"

max_response_time = 60        # seconds
verify_ssl = true
```

## Error Handling

### Network Errors
- Connection refused/timeout
- DNS resolution failures  
- SSL certificate validation errors
- HTTP error status codes (4xx, 5xx)

### Content Errors
- Malformed HTML content
- Encoding detection failures
- Content size exceeded limits
- Invalid Content-Type headers

### Security Errors
- Blocked domains or URLs
- SSRF attempt detection
- Rate limit violations
- Invalid redirect chains

## Integration Points

### Workflow Integration
- Web content can be stored in workflow variables
- Support for conditional fetching based on workflow state
- Integration with prompt templates for content processing
- Error handling integrates with workflow abort mechanisms

### Logging Integration
- Structured logging with request/response metadata
- Error categorization and reporting

## Inspiration and References

### Cline WebFetch Tool
Based on the web fetch implementation from [Cline](https://github.com/cline/cline/blob/main/src/core/tools/webFetchTool.ts), this specification adapts the TypeScript concept for Rust/MCP environment with enhanced features:

- Rust-native HTTP client using `reqwest` through `markdowndown`
- Advanced HTML-to-markdown conversion capabilities
- Enhanced security controls for server environments
- Structured metadata extraction and reporting

### MarkdownDown Integration
Leverages the [SwissArmyHammer MarkdownDown](https://github.com/swissarmyhammer/markdowndown) crate for:
- Production-ready HTTP client with proper error handling
- High-quality HTML-to-markdown conversion
- Built-in security features and content validation
- Configurable processing options

## Testing Strategy

### Unit Tests
- URL validation and sanitization
- HTTP response handling for various status codes
- Markdown conversion quality verification
- Error condition simulation and handling

### Integration Tests
- Real-world website fetching scenarios
- Redirect chain handling verification
- Timeout and error recovery testing
- Security control validation

### Performance Tests
- Large content handling and memory usage
- Concurrent request processing

### Security Tests
- SSL certificate validation
