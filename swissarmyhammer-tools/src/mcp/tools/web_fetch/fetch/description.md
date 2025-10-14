Fetch web content and convert HTML to markdown for AI processing.

## Parameters

- `url` (required): The URL to fetch content from (HTTP/HTTPS only)
- `timeout` (optional): Request timeout in seconds (default: 30, min: 5, max: 120)
- `follow_redirects` (optional): Whether to follow HTTP redirects (default: true)
- `max_content_length` (optional): Maximum content length in bytes (default: 1MB, max: 10MB)
- `user_agent` (optional): Custom User-Agent header (default: "SwissArmyHammer-Bot/1.0")

## Examples

```json
{
  "url": "https://example.com/page",
  "timeout": 60
}
```

## Returns

Returns converted markdown content. On error, returns error type and details.
