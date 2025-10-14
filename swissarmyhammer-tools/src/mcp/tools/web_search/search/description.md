Perform web searches using DuckDuckGo with optional content fetching.

## Parameters

- `query` (required): The search query string (1-500 characters)
- `category` (optional): Search category - "general", "images", "videos", "news", "it", etc. (default: "general")
- `language` (optional): Search language code (default: "en")
- `results_count` (optional): Number of results to return (default: 10, max: 50)
- `fetch_content` (optional): Fetch and convert page content to markdown (default: true)
- `safe_search` (optional): Safe search level - 0 (off), 1 (moderate), 2 (strict) (default: 1)
- `time_range` (optional): Time range filter - "", "day", "week", "month", "year" (default: "")

## Examples

```json
{
  "query": "rust async programming",
  "category": "it",
  "results_count": 15
}
```

## Returns

Returns array of search results with titles, URLs, descriptions, scores, and optional converted markdown content. Includes metadata with query, search time, and fetch statistics.
