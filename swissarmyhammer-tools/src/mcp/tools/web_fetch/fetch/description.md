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

## Response Format

### Successful Fetch Response
When content is successfully fetched, the tool returns only the converted markdown content:

**Response Content:** The actual fetched content converted to markdown format.

**Example Response:**
```
# Example Page Title

This is the converted markdown content from the fetched webpage...

## Section Heading

Content paragraphs and other markdown elements are preserved in the conversion.
```

**Key Benefits:**
- Clean content delivery without technical metadata
- Direct markdown format ready for AI processing  
- No verbose announcements or performance metrics
- Redirects handled transparently by the underlying library

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
