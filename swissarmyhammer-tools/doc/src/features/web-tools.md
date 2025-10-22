# Web Tools

SwissArmyHammer provides web tools for fetching content and searching the web, with markdown conversion for AI-friendly consumption.

## Overview

Web tools enable AI assistants to access external information, fetch documentation, and search the web while converting HTML to markdown for easy processing.

## Available Tools

### web_fetch

Fetch web content and convert HTML to markdown for AI processing.

**Parameters:**
- `url` (required): The URL to fetch content from (HTTP/HTTPS only)
- `timeout` (optional): Request timeout in seconds (default: 30, min: 5, max: 120)
- `follow_redirects` (optional): Whether to follow HTTP redirects (default: true)
- `max_content_length` (optional): Maximum content length in bytes (default: 1MB, max: 10MB)
- `user_agent` (optional): Custom User-Agent header (default: "SwissArmyHammer-Bot/1.0")

**Example:**
```json
{
  "url": "https://example.com/docs",
  "timeout": 60
}
```

**Returns:** Converted markdown content

---

### web_search

Perform web searches using DuckDuckGo with optional content fetching.

**Parameters:**
- `query` (required): The search query string (1-500 characters)
- `category` (optional): Search category - "general", "images", "videos", "news", "it", etc. (default: "general")
- `language` (optional): Search language code (default: "en")
- `results_count` (optional): Number of results to return (default: 10, max: 50)
- `fetch_content` (optional): Fetch and convert page content to markdown (default: true)
- `safe_search` (optional): Safe search level - 0 (off), 1 (moderate), 2 (strict) (default: 1)
- `time_range` (optional): Time range filter - "", "day", "week", "month", "year" (default: "")

**Example:**
```json
{
  "query": "rust async programming",
  "category": "it",
  "results_count": 15
}
```

**Returns:** Array of search results with titles, URLs, descriptions, scores, and optional converted markdown content

## Web Fetch Use Cases

### Fetching Documentation

Retrieve documentation pages:

```json
{
  "url": "https://doc.rust-lang.org/book/ch01-00-getting-started.html"
}
```

Use for:
- Learning new APIs
- Referencing examples
- Following tutorials

### Reading Articles

Fetch technical articles:

```json
{
  "url": "https://blog.rust-lang.org/2024/01/01/article.html",
  "timeout": 60
}
```

Use for:
- Research
- Best practices
- Design patterns

### Accessing API Documentation

Fetch API reference pages:

```json
{
  "url": "https://api.example.com/v1/reference"
}
```

Use for:
- Integration work
- Understanding endpoints
- Learning parameters

### Retrieving Specifications

Fetch specifications and RFCs:

```json
{
  "url": "https://www.rfc-editor.org/rfc/rfc9110.html"
}
```

Use for:
- Protocol implementation
- Standards compliance
- Design decisions

## Web Search Use Cases

### Finding Documentation

Search for specific documentation:

```json
{
  "query": "rust tokio async tutorial",
  "category": "it",
  "results_count": 10,
  "fetch_content": true
}
```

### Research Solutions

Find solutions to problems:

```json
{
  "query": "how to implement jwt authentication rust",
  "category": "it",
  "results_count": 15
}
```

### Finding Libraries

Discover relevant libraries:

```json
{
  "query": "rust vector database library",
  "category": "it"
}
```

### Learning Best Practices

Search for best practices:

```json
{
  "query": "rust error handling best practices 2024",
  "category": "it",
  "time_range": "year"
}
```

## Search Categories

- **general**: General web search across all content
- **images**: Search for images
- **videos**: Search for videos
- **news**: Search for news articles
- **map**: Search for locations
- **music**: Search for music content
- **it**: Search for IT and technology content
- **science**: Search for scientific content
- **files**: Search for files and documents

## Content Fetching

### Automatic Fetching

When `fetch_content: true`, search results include:
- Converted markdown content
- Full page text
- Cleaned HTML structure

### Manual Fetching

When `fetch_content: false`, only metadata:
- Title
- URL
- Description
- Score

Then use `web_fetch` to retrieve specific pages.

## Markdown Conversion

HTML is converted to markdown with:
- Headers preserved
- Links maintained
- Code blocks extracted
- Lists formatted
- Tables converted
- Images noted

This makes content easy for AI to process.

## Integration Patterns

### Research and Save

1. Search: `web_search`
2. Fetch pages: `web_fetch`
3. Save: `memo_create`
4. Reference later: `memo_get`

### Documentation Discovery

1. Search: `web_search`
2. Identify best results
3. Fetch content: `web_fetch`
4. Use in development

### Compare Solutions

1. Search multiple times
2. Fetch all results
3. Compare approaches
4. Choose best solution

### Build Knowledge Base

1. Search topic
2. Fetch relevant pages
3. Create memos for each
4. Reference in code comments

## Best Practices

### Search Queries

1. **Be Specific**: Use precise terms
2. **Include Context**: Add language/framework
3. **Filter by Time**: Use recent results
4. **Use Categories**: Narrow with category filter

### Fetching Content

1. **Check URLs**: Verify URLs are correct
2. **Set Timeouts**: Adjust for slow sites
3. **Handle Failures**: Not all fetches succeed
4. **Respect Limits**: Don't fetch excessively

### Privacy

1. **No Tracking**: DuckDuckGo doesn't track
2. **User Agent**: Identifies as SwissArmyHammer
3. **No Cookies**: No persistent state
4. **HTTPS Only**: Secure connections only

## Rate Limiting

### DuckDuckGo

DuckDuckGo may rate limit excessive searches:
- Pace searches reasonably
- Don't automate rapid searches
- Respect search engine policies

### Web Fetch

Web servers may rate limit:
- Don't fetch repeatedly
- Cache results when possible
- Respect robots.txt

## Error Handling

### Fetch Errors

Common errors:
- **Network Timeout**: Increase timeout
- **Connection Refused**: Server unavailable
- **Invalid URL**: Check URL format
- **Content Too Large**: Reduce max_content_length

### Search Errors

Common errors:
- **No Results**: Try different terms
- **Rate Limited**: Wait before searching again
- **Invalid Query**: Check query format

## Security Considerations

### URL Validation

- Only HTTP/HTTPS supported
- URLs validated before fetching
- Redirects followed safely
- Content length limits enforced

### Content Processing

- HTML sanitized during conversion
- Scripts removed
- Potentially harmful content filtered

### Network Safety

- HTTPS preferred
- Timeouts prevent hanging
- Size limits prevent memory issues

## Performance Considerations

- **Fetch Speed**: Depends on remote server
- **Search Speed**: Depends on DuckDuckGo
- **Content Size**: Large pages take longer
- **Network**: Affected by connection speed

## Examples

### Simple Documentation Fetch

```json
{
  "url": "https://docs.rs/tokio/latest/tokio/"
}
```

### Comprehensive Search

```json
{
  "query": "rust async streams tutorial",
  "category": "it",
  "results_count": 20,
  "fetch_content": true,
  "language": "en",
  "time_range": "year"
}
```

### Quick Reference Check

```json
{
  "query": "rust std vec api",
  "category": "it",
  "results_count": 5,
  "fetch_content": false
}
```

Then fetch specific result:
```json
{
  "url": "https://doc.rust-lang.org/std/vec/struct.Vec.html"
}
```

## Limitations

### Fetch Limitations

- No JavaScript execution
- No dynamic content
- Static HTML only
- No form submission

### Search Limitations

- Results depend on DuckDuckGo
- No custom search engines
- No advanced operators
- Limited to 50 results

### Content Limitations

- Markdown conversion is best-effort
- Complex layouts may not convert well
- Tables may be simplified
- Images only noted, not downloaded

## Troubleshooting

### Fetch Fails

**Issue:** Can't fetch URL.

**Solution:**
- Verify URL is correct
- Check network connectivity
- Increase timeout
- Try with curl/browser first

### Search No Results

**Issue:** Search returns nothing.

**Solution:**
- Try different search terms
- Broaden query
- Remove time filter
- Try different category

### Content Garbled

**Issue:** Converted markdown is messy.

**Solution:**
- This is expected for complex layouts
- Focus on main content
- Try fetching specific sections
- Use direct URL if available

## Next Steps

- [Semantic Search](./semantic-search.md): Search local code
- [File Operations](./file-operations.md): Work with files
- [Issue Management](./issue-management.md): Track research tasks
