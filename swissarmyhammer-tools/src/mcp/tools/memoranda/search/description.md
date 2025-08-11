Search memos by query string. Searches both title and content for matches.

## Parameters

- `query` (required): Search query string to match against memo titles and content

## Examples

Search for memos containing specific text:
```json
{
  "query": "meeting notes project"
}
```

## Returns

Returns a list of memos that match the search query, including their titles, IDs, and content excerpts with matching terms highlighted.