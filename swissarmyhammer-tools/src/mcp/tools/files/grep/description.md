Content-based search with ripgrep for fast text searching.

## Parameters

- `pattern` (required): Regular expression pattern to search
- `path` (optional): File or directory to search in (defaults to current directory)
- `glob` (optional): Glob pattern to filter files (e.g., `*.js`)
- `type` (optional): File type filter (e.g., `js`, `py`, `rust`)
- `case_insensitive` (optional): Case-insensitive search (default: false)
- `context_lines` (optional): Number of context lines around matches (default: 0)
- `output_mode` (optional): Output format - `content`, `files_with_matches`, or `count` (default: content)

## Examples

```json
{
  "pattern": "fn\\s+\\w+\\s*\\(",
  "type": "rust"
}
```

## Returns

Returns matches with file paths, line numbers, and content. Format depends on output_mode.
