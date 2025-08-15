# Web Fetch Tool

Fetch web content and convert HTML to markdown for AI processing. Leverages the `markdowndown` crate for high-quality HTML-to-markdown conversion with comprehensive redirect handling, security controls, and configurable limits.

## Parameters

### Required Parameters

- **`url`** (string, format: uri): The URL to fetch content from
  - Must be a valid HTTP/HTTPS URL
  - Only HTTP and HTTPS protocols are supported
  - URL validation performed to prevent SSRF attacks

### Optional Parameters

- **`timeout`** (integer): Request timeout in seconds
  - Default: 30 seconds
  - Minimum: 5 seconds  
  - Maximum: 120 seconds
  - Controls total request time including redirects

- **`follow_redirects`** (boolean): Whether to follow HTTP redirects
  - Default: true
  - When true: Follows up to 10 redirects maximum
  - When false: Stops at first redirect response
  - Tracks complete redirect chain for transparency

- **`max_content_length`** (integer): Maximum content length in bytes
  - Default: 1,048,576 bytes (1MB)
  - Minimum: 1,024 bytes (1KB)
  - Maximum: 10,485,760 bytes (10MB)
  - Prevents memory exhaustion from large responses

- **`user_agent`** (string): Custom User-Agent header
  - Default: "SwissArmyHammer-Bot/1.0"
  - Used for server identification and request tracking
  - Should identify your application appropriately

## Usage Examples

### Basic Content Retrieval
Fetch documentation or web pages with default settings:
```json
{
  "url": "https://docs.rust-lang.org/book/ch04-01-what-is-ownership.html"
}
```

### Documentation Research
Fetch technical documentation with extended timeout and content limits:
```json
{
  "url": "https://docs.rust-lang.org/book/ch04-01-what-is-ownership.html",
  "timeout": 45,
  "max_content_length": 2097152
}
```

### API Documentation Processing
Retrieve API documentation with custom user agent:
```json
{
  "url": "https://api.github.com/docs/rest/repos",
  "user_agent": "SwissArmyHammer-DocProcessor/1.0",
  "timeout": 60
}
```

### Content Validation
Quick content checks with reduced timeout:
```json
{
  "url": "https://example.com/changelog",
  "follow_redirects": true,
  "timeout": 15
}
```

### News and Content Analysis  
Large content processing with maximum limits:
```json
{
  "url": "https://blog.rust-lang.org/2024/01/15/recent-updates.html",
  "max_content_length": 5242880,
  "timeout": 90
}
```

### Redirect Chain Analysis
Fetch with redirect chain disabled for redirect analysis:
```json
{
  "url": "https://short.link/redirect-example", 
  "follow_redirects": false,
  "timeout": 10
}
```

## Response Formats

### Successful Fetch Response
When content is successfully fetched and converted to markdown:
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
      "content-encoding": "gzip",
      "last-modified": "Wed, 01 Jan 2025 12:00:00 GMT"
    }
  }
}
```

**Key Response Fields:**
- `url`: Original requested URL
- `final_url`: Final URL after redirects (same as url if no redirects)
- `title`: Extracted HTML page title
- `content_type`: Response Content-Type header
- `content_length`: Size of content in bytes
- `status_code`: HTTP status code (200 for success)
- `response_time_ms`: Total request duration in milliseconds
- `markdown_content`: Converted markdown content
- `word_count`: Number of words in converted content
- `headers`: Selected HTTP response headers

### Redirect Response Format
When redirects are followed (`follow_redirects: true`):
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
    "response_time_ms": 432,
    "markdown_content": "# Redirected Page Content...",
    "redirect_chain": [
      "https://example.com/old-page -> 301",
      "https://example.com/temp-page -> 302", 
      "https://example.com/new-page -> 200"
    ],
    "title": "Final Page Title",
    "content_type": "text/html",
    "word_count": 1245
  }
}
```

**Additional Redirect Fields:**
- `redirect_count`: Number of redirects followed
- `redirect_chain`: Complete chain of URLs and status codes
- `final_url`: URL where content was actually fetched

### Error Response Format
When requests fail or encounter errors:
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

**Common Error Types:**
- `timeout`: Request exceeded configured timeout
- `connection_failed`: Unable to establish connection
- `dns_resolution`: Domain name resolution failed
- `ssl_error`: SSL certificate validation failed
- `http_error`: HTTP error status (4xx, 5xx)
- `content_too_large`: Response exceeds max_content_length
- `invalid_url`: URL format validation failed
- `security_violation`: Request blocked by security policies

## Redirect Handling

- **Maximum Redirects**: Enforces 10 redirect limit when `follow_redirects: true`, 0 when `false`
- **Chain Tracking**: Complete tracking of all redirect steps with URLs and status codes
- **Relative URL Resolution**: Properly resolves relative redirect locations against base URLs
- **Status Code Support**: Handles all redirect types (301, 302, 303, 307, 308)
- **Loop Prevention**: Detects and prevents infinite redirect loops
- **Response Metadata**: Includes redirect count and chain in successful responses

## Security Considerations

### URL Validation and SSRF Protection
- **Protocol Restriction**: Only HTTP and HTTPS protocols are accepted
- **URL Format Validation**: Comprehensive URL syntax validation before requests
- **Server-Side Request Forgery (SSRF) Protection**: Built-in safeguards against internal network access
- **Domain Validation**: Configurable domain restrictions and blacklists
- **IP Address Filtering**: Protection against private network and localhost access

### SSL/TLS Security
- **Certificate Validation**: Strict SSL certificate verification for HTTPS requests
- **TLS Version Requirements**: Modern TLS versions enforced
- **Certificate Chain Validation**: Complete certificate chain verification
- **Certificate Authority Validation**: Only trusted CA certificates accepted

### Content Security Controls  
- **Content-Type Verification**: Validates response content types match expectations
- **Size Limits**: Configurable maximum content size prevents memory exhaustion
- **Response Header Validation**: Security-focused header inspection
- **Content Encoding Support**: Secure handling of compressed content

### Rate Limiting and Abuse Prevention
- **Request Rate Limiting**: Built-in protection against excessive requests
- **Circuit Breaker Pattern**: Automatic protection for problematic domains
- **Timeout Controls**: Prevents resource exhaustion from hanging requests
- **Concurrent Request Limits**: Controls simultaneous request processing

### Redirect Security
- **Redirect Loop Prevention**: Detects and prevents infinite redirect chains  
- **Maximum Redirect Limits**: Enforces strict redirect count limits (max 10)
- **Redirect Chain Validation**: Validates each step in redirect chains
- **Relative URL Resolution Security**: Safe resolution of relative redirect URLs

## Security Best Practices

### Safe Usage Guidelines
1. **Validate URLs**: Always validate URLs from untrusted sources before use
2. **Use Appropriate Timeouts**: Set reasonable timeouts for your use case
3. **Limit Content Size**: Configure max_content_length based on memory constraints
4. **Monitor Request Patterns**: Track request patterns for abuse detection
5. **Use Custom User-Agents**: Identify your application appropriately

### Configuration Recommendations
```json
{
  "url": "https://trusted-domain.com/content",
  "timeout": 30,
  "max_content_length": 1048576,
  "user_agent": "YourApplication/1.0 (contact@yourapp.com)",
  "follow_redirects": true
}
```

### Error Handling for Security
- Always check `is_error` field in responses
- Log security violations for monitoring
- Handle rate limiting gracefully with backoff
- Validate content before processing

### Domain Trust Considerations
- **Public Domains**: Generally safe but validate content
- **Internal Domains**: Blocked by SSRF protection by default
- **Short URLs**: Use with caution, consider redirect analysis
- **User-Provided URLs**: Always validate and sanitize

### Compliance and Privacy
- **Request Logging**: Configure appropriate logging levels
- **User-Agent Identification**: Use responsible identification strings
- **Respect robots.txt**: Consider implementing robots.txt checking
- **Data Retention**: Configure appropriate content retention policies

## Common Use Cases and Integration Patterns

### Documentation Research and Analysis
Fetch technical documentation for analysis and integration planning:

**Use Case**: Researching Rust ownership concepts
```json
{
  "url": "https://docs.rust-lang.org/book/ch04-01-what-is-ownership.html",
  "timeout": 45,
  "max_content_length": 2097152,
  "user_agent": "DocumentationAnalyzer/1.0"
}
```

**Integration Pattern**: Store fetched content in workflow variables for further processing
- Use the `markdown_content` field for AI analysis
- Extract `title` and `word_count` for content categorization
- Track `response_time_ms` for performance monitoring

### API Documentation Processing
Retrieve and process API documentation for development workflows:

**Use Case**: GitHub API documentation analysis
```json
{
  "url": "https://docs.github.com/en/rest/repos/repos",
  "timeout": 60,
  "user_agent": "APIDocProcessor/2.0"
}
```

**Workflow Integration**: 
- Parse API endpoints from markdown content
- Extract parameter documentation
- Generate client code from API specifications
- Update documentation with latest API changes

### Content Validation Workflows
Implement content validation and fact-checking pipelines:

**Use Case**: Changelog validation
```json
{
  "url": "https://project.example.com/CHANGELOG.md",
  "follow_redirects": true,
  "timeout": 15
}
```

**Error Handling Pattern**:
```json
// Check if content fetch was successful
if (!response.is_error) {
  // Process response.metadata.markdown_content
  // Validate against expected patterns
  // Extract version information
} else {
  // Handle error based on response.metadata.error_type
  // Log error for monitoring
  // Implement fallback strategy
}
```

### News and Content Analysis
Process news articles and blog content for analysis:

**Use Case**: Tech blog analysis
```json
{
  "url": "https://blog.rust-lang.org/2024/01/15/recent-updates.html",
  "max_content_length": 5242880,
  "timeout": 90
}
```

**Integration with Prompt Templates**:
- Pass `markdown_content` to analysis prompts
- Use `title` for content categorization
- Leverage `word_count` for content complexity assessment

### Redirect Chain Analysis
Analyze URL redirect patterns for security and SEO:

**Use Case**: Short URL analysis  
```json
{
  "url": "https://bit.ly/example-link",
  "follow_redirects": true,
  "timeout": 20
}
```

**Analysis Pattern**:
- Examine `redirect_chain` for suspicious patterns
- Validate `final_url` against expected destinations
- Monitor `redirect_count` for performance impact

### Batch Content Processing
Process multiple URLs in sequence with error handling:

**Workflow Pattern**:
```json
[
  {
    "url": "https://docs.example.com/guide1.html",
    "timeout": 30
  },
  {
    "url": "https://docs.example.com/guide2.html", 
    "timeout": 30
  }
]
```

**Error Recovery Strategy**:
- Continue processing remaining URLs on individual failures
- Aggregate successful results
- Report failed URLs for retry
- Implement exponential backoff for rate limiting

### Conditional Content Fetching
Implement conditional fetching based on workflow state:

**Use Case**: Fetch only if content has changed
```json
{
  "url": "https://api.example.com/status",
  "timeout": 10,
  "user_agent": "StatusMonitor/1.0"
}
```

**State Management**:
- Store previous `content_length` and `last_modified` headers
- Compare with current fetch results
- Process only when content changes detected
- Maintain fetch history for trend analysis

## Performance Considerations

### Memory Management
- **Content Size Limits**: Configure `max_content_length` based on available memory
- **Streaming Processing**: Large content is processed in chunks to minimize memory usage
- **Garbage Collection**: Automatic cleanup of temporary data structures
- **Memory Monitoring**: Track memory usage through response metadata

### Timeout Configuration Strategy
```json
{
  "timeout": 30,  // Balance between reliability and performance
  // Recommended timeout ranges:
  // - Fast APIs: 5-15 seconds
  // - Documentation: 30-60 seconds  
  // - Large content: 60-120 seconds
}
```

### Concurrent Request Patterns
- **Sequential Processing**: Process URLs one at a time for simple workflows
- **Batch Processing**: Group related requests for efficiency
- **Rate Limiting**: Respect server rate limits and implement backoff
- **Connection Pooling**: Reuse HTTP connections when possible

### Optimization Recommendations

#### For High-Volume Processing
```json
{
  "timeout": 15,
  "max_content_length": 512000,  // 500KB for faster processing
  "follow_redirects": false,      // Reduce redirect overhead  
  "user_agent": "HighVolume/1.0"
}
```

#### For Large Content Analysis
```json
{
  "timeout": 90,
  "max_content_length": 5242880,  // 5MB for comprehensive content
  "follow_redirects": true,
  "user_agent": "ContentAnalyzer/1.0"
}
```

#### For Redirect Analysis
```json
{
  "timeout": 20,
  "max_content_length": 102400,  // 100KB, content not needed
  "follow_redirects": true,
  "user_agent": "RedirectAnalyzer/1.0"
}
```

### Performance Monitoring
- **Response Time Tracking**: Monitor `response_time_ms` for performance baseline
- **Content Size Monitoring**: Track `content_length` for capacity planning  
- **Redirect Count Analysis**: Monitor `redirect_count` for performance impact
- **Error Rate Monitoring**: Track error patterns for system health

### Troubleshooting Performance Issues

#### Slow Responses
1. Check `response_time_ms` in metadata
2. Verify timeout configuration is appropriate
3. Monitor network connectivity to target domain
4. Consider reducing `max_content_length` if not needed

#### Memory Issues
1. Reduce `max_content_length` parameter
2. Process smaller batches of URLs
3. Monitor system memory usage during operations
4. Implement content size checks before processing

#### Rate Limiting
1. Monitor for `http_error` responses with 429 status
2. Implement exponential backoff strategy
3. Reduce request frequency
4. Use appropriate `user_agent` identification

## Integration with MarkdownDown

The tool leverages the `markdowndown` crate for:
- **High-Performance HTTP Client**: Optimized reqwest-based HTTP client
- **Content-Type Detection**: Automatic character encoding handling
- **HTML-to-Markdown Conversion**: High-quality conversion with customizable options
- **Built-in Security Features**: URL validation and content sanitization
- **Memory-Efficient Processing**: Streaming conversion for large content

### Conversion Quality Features
- **Preserve Code Blocks**: Maintains syntax highlighting information
- **Table Conversion**: Converts HTML tables to markdown format
- **Link Preservation**: Maintains all link relationships
- **Heading Structure**: Preserves document hierarchy
- **List Processing**: Converts ordered and unordered lists accurately

## Comprehensive Error Handling

The tool provides detailed error handling for all failure scenarios:

### Network-Level Errors
- **Connection Failures**: TCP connection refused, network unreachable
- **DNS Resolution**: Domain name resolution failures and timeouts
- **SSL/TLS Errors**: Certificate validation, cipher negotiation failures
- **Timeout Handling**: Request timeout with partial data recovery where possible

### HTTP-Level Errors  
- **Client Errors (4xx)**: Invalid requests, authentication failures, not found
- **Server Errors (5xx)**: Server failures, service unavailable, gateway errors
- **Redirect Errors**: Malformed redirect responses, redirect loops
- **Content Errors**: Malformed HTML, unsupported encodings, size limits exceeded

### Security and Validation Errors
- **URL Validation**: Invalid URL format, unsupported protocols
- **SSRF Protection**: Internal network access attempts blocked
- **Content Validation**: Suspicious content patterns detected
- **Rate Limiting**: Request rate exceeded, temporary blocks

### Error Response Structure
All errors include structured metadata for programmatic handling:
- **Error Classification**: Specific error type for appropriate handling
- **Detailed Messages**: Human-readable error descriptions
- **Timing Information**: Request duration before failure
- **Context Data**: Additional context for debugging and monitoring

## Tool Response Summary

The web_fetch tool returns comprehensive structured responses containing:

### Success Responses
- **Status Information**: Success flags and HTTP status codes
- **Content Data**: High-quality markdown conversion of HTML content
- **Metadata**: Complete request/response information including timing
- **Content Analysis**: Word counts, title extraction, content metrics
- **Redirect Information**: Complete redirect chain tracking when applicable

### Error Responses  
- **Error Classification**: Structured error types for programmatic handling
- **Diagnostic Information**: Detailed error context for debugging
- **Timing Data**: Request duration and timeout information
- **Recovery Guidance**: Suggested actions for error resolution