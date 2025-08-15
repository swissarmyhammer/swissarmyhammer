# Web Fetch Tool

Fetch web content and convert HTML to markdown for AI processing. Returns ranked results based on semantic similarity to the query.

## Parameters

- `url` (required): The URL to fetch content from (must be a valid HTTP/HTTPS URL)
- `timeout` (optional): Request timeout in seconds (default: 30, min: 5, max: 120)
- `follow_redirects` (optional): Whether to follow HTTP redirects (default: true)
- `max_content_length` (optional): Maximum content length in bytes (default: 1MB, min: 1KB, max: 10MB)
- `user_agent` (optional): Custom User-Agent header (default: "SwissArmyHammer-Bot/1.0")

## Examples

Basic web content fetch:
```json
{
  "url": "https://docs.rust-lang.org/book/ch04-01-what-is-ownership.html"
}
```

Advanced fetch with custom settings:
```json
{
  "url": "https://api.github.com/docs/rest/repos",
  "timeout": 45,
  "follow_redirects": true,
  "max_content_length": 2097152,
  "user_agent": "SwissArmyHammer-DocProcessor/1.0"
}
```

Documentation analysis:
```json
{
  "url": "https://example.com/changelog",
  "timeout": 15,
  "max_content_length": 5242880
}
```

## Successful Response Format

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

## Redirect Response Format

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

## Error Response Format

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

## Security Features

- **URL Validation**: Only HTTP/HTTPS URLs accepted
- **Content-Type Verification**: Validates response content types
- **Rate Limiting**: Prevents abuse with configurable limits
- **Size Limits**: Configurable maximum content size to prevent memory issues
- **Redirect Protection**: Limits redirect chains to prevent infinite loops
- **SSL Verification**: Validates SSL certificates for HTTPS connections

## Use Cases

### Documentation Research
Perfect for fetching and analyzing documentation pages, converting HTML to clean markdown for AI processing.

### API Documentation Processing
Retrieve API documentation and convert it to structured markdown for analysis and integration planning.

### Content Validation
Fetch web content for validation, fact-checking, and content analysis workflows.

### News and Content Analysis
Retrieve and process news articles, blog posts, and other web content for analysis and summarization.

## Integration with MarkdownDown

The tool leverages the `markdowndown` crate for:
- HTML fetching with proper HTTP client configuration
- Content-Type detection and character encoding handling
- HTML-to-markdown conversion with customizable options
- Built-in security features and URL validation

## Error Handling

The tool provides comprehensive error handling for:
- **Network Errors**: Connection refused, timeout, DNS resolution failures
- **HTTP Errors**: 4xx and 5xx status codes with detailed error information
- **Content Errors**: Malformed HTML, encoding issues, size limit exceeded
- **Security Errors**: Blocked domains, invalid redirects, SSL certificate issues

## Returns

Returns structured response with:
- Success/error status
- Converted markdown content
- Comprehensive metadata (timing, headers, redirects)
- Error details when applicable
- Content analysis (word count, title extraction)