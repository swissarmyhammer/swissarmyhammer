# Web Search Tool

Perform web searches using SearXNG metasearch engines with privacy protection and optional content fetching.

## Purpose

The web search tool enables LLMs to search the web for current information, technical documentation, fact-checking, and research tasks. It uses SearXNG instances to provide privacy-respecting search capabilities without tracking user queries.

## Parameters

### Required Parameters

- `query` (string): The search query string
  - Minimum length: 1 character
  - Maximum length: 500 characters
  - Example: "rust async programming"

### Optional Parameters

- `category` (string): Search category for filtering results
  - Options: "general", "images", "videos", "news", "map", "music", "it", "science", "files"
  - Default: "general"
  - Example: "it" for technical searches

- `language` (string): Search language code
  - Format: ISO 639-1 language code (e.g., "en", "fr", "de")
  - Pattern: `^[a-z]{2}(-[A-Z]{2})?$`
  - Default: "en"
  - Example: "en" for English results

- `results_count` (integer): Number of search results to return
  - Range: 1 to 50
  - Default: 10
  - Example: 15

- `fetch_content` (boolean): Whether to fetch and process content from result URLs
  - Default: true
  - When true: Fetches page content and converts to markdown
  - When false: Returns only search result metadata

- `safe_search` (integer): Safe search filtering level
  - 0: Off (no filtering)
  - 1: Moderate filtering (default)
  - 2: Strict filtering
  - Default: 1

- `time_range` (string): Time range filter for results
  - Options: "", "day", "week", "month", "year"
  - Default: "" (all time)
  - Example: "month" for results from the last month

## Response Format

### Successful Search Response

Returns a structured response with search results and metadata:

```json
{
  "results": [
    {
      "title": "Page Title",
      "url": "https://example.com/page",
      "description": "Page description or snippet",
      "score": 0.95,
      "engine": "duckduckgo",
      "content": {
        "markdown": "# Page Content\n\nConverted to markdown...",
        "word_count": 1500,
        "fetch_time_ms": 340,
        "summary": "Brief summary of the content"
      }
    }
  ],
  "metadata": {
    "query": "rust async programming",
    "category": "general",
    "language": "en",
    "results_count": 10,
    "search_time_ms": 1250,
    "instance_used": "https://search.example.org",
    "total_results": 8450,
    "engines_used": ["duckduckgo", "google", "bing"],
    "fetch_content": true,
    "content_fetch_stats": {
      "attempted": 10,
      "successful": 8,
      "failed": 2,
      "total_time_ms": 2840
    }
  }
}
```

### Error Response

Returns error information when search operations fail:

```json
{
  "error_type": "no_instances_available",
  "error_details": "All configured SearXNG instances are unavailable or rate limited",
  "attempted_instances": ["https://search.example1.org", "https://search.example2.org"],
  "retry_after": 300
}
```

## Use Cases

### Technical Research
Search for programming documentation, tutorials, and technical resources:
```json
{
  "query": "rust async sqlx database connection pooling",
  "category": "it",
  "results_count": 15,
  "fetch_content": true,
  "time_range": "year"
}
```

### Current Events and News
Find recent news and developments:
```json
{
  "query": "latest rust language updates 2024",
  "category": "news",
  "results_count": 10,
  "time_range": "month"
}
```

### Documentation Discovery
Search specific documentation sites:
```json
{
  "query": "site:docs.rs serde deserialize custom format",
  "category": "general",
  "results_count": 8,
  "fetch_content": true
}
```

### Quick Fact Checking
Get quick answers without full content fetching:
```json
{
  "query": "rust 1.75 release date new features",
  "results_count": 5,
  "fetch_content": false
}
```

## Privacy and Security Features

- **Privacy Protection**: Uses SearXNG instances that don't track users
- **Instance Rotation**: Distributes requests across multiple instances
- **Anonymous Requests**: No persistent user identification
- **Secure Communication**: All requests use HTTPS
- **Rate Limiting**: Respects instance limits and implements backoff

## Performance Considerations

- Search operations typically complete in 1-3 seconds
- Content fetching adds 2-5 seconds depending on target sites
- Concurrent content fetching with rate limiting
- Automatic fallback to alternative instances
- Graceful degradation when content fetching fails

## Error Handling

The tool handles various error conditions gracefully:

- **Instance Unavailability**: Automatic failover to backup instances
- **Rate Limiting**: Respects rate limits with exponential backoff
- **Content Fetch Failures**: Returns search results even if content fetching fails
- **Invalid Parameters**: Clear validation error messages
- **Network Issues**: Timeout handling with retry logic

## Integration

The web search tool integrates seamlessly with other MCP tools:

- **Memo Creation**: Search results can be saved to memos
- **Issue Research**: Use search results for issue investigation
- **Workflow Integration**: Chain searches with other research activities
- **Content Analysis**: Process fetched content with other tools