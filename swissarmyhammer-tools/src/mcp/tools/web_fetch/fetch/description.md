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
