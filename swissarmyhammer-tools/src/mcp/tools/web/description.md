Web operations for searching and fetching content.

## Operations

- **search url**: Search the web using DuckDuckGo with optional content fetching
- **fetch url**: Fetch a specific URL and convert HTML to markdown

## Examples

```json
{"op": "search url", "query": "rust async programming", "results_count": 10}
```

```json
{"op": "fetch url", "url": "https://example.com/page"}
```
