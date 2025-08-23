# Web Search Tool

Perform comprehensive web searches using DuckDuckGo with privacy protection and optional content fetching.

## Purpose

The web search tool enables LLMs to search the web for current information, technical documentation, fact-checking, and research tasks. It uses DuckDuckGo's web search interface to provide actual web search results (not just instant answers) with privacy-respecting capabilities and no query tracking.

## Important Limitations

**Bot Detection**: DuckDuckGo may occasionally detect automated requests and require CAPTCHA verification. When this happens, you'll receive a clear error message explaining the situation. This is a protective measure by DuckDuckGo to prevent abuse. If you encounter CAPTCHA challenges:

- Wait a few minutes before retrying
- The tool uses human-like User-Agent strings to minimize detection
- Consider using the DuckDuckGo web interface directly for immediate access

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
    "instance_used": "https://html.duckduckgo.com",
    "total_results": 8450,
    "engines_used": ["duckduckgo"],
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
  "error_type": "captcha_required",
  "error_details": "DuckDuckGo is requesting CAPTCHA verification. This is a bot protection measure. Please try again later or reduce request frequency.",
  "attempted_instances": ["https://html.duckduckgo.com"],
  "retry_after": 60
}
```
