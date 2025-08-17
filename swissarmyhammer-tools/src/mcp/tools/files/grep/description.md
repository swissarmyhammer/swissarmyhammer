Content-based search using ripgrep for fast and flexible text searching.

## Parameters

- `pattern` (required): Regular expression pattern to search
- `path` (optional): File or directory to search in
- `glob` (optional): Glob pattern to filter files (e.g., `*.js`)
- `type` (optional): File type filter (e.g., `js`, `py`, `rust`)
- `case_insensitive` (optional): Case-insensitive search
- `context_lines` (optional): Number of context lines around matches
- `output_mode` (optional): Output format (`content`, `files_with_matches`, `count`)

## Functionality

- Leverages ripgrep for high-performance text search
- Supports full regular expression syntax
- Provides file type and glob filtering
- Returns contextual information around matches
- Handles large codebases efficiently

## Use Cases

- Finding function definitions or usages
- Searching for specific code patterns
- Locating configuration values
- Identifying potential issues or code smells

## Examples

Find function definitions:
```json
{
  "pattern": "fn\\s+\\w+\\s*\\(",
  "type": "rust"
}
```

Search for TODO comments with context:
```json
{
  "pattern": "TODO|FIXME",
  "case_insensitive": true,
  "context_lines": 2,
  "output_mode": "content"
}
```

Find files containing specific imports:
```json
{
  "pattern": "import.*React",
  "glob": "**/*.{js,jsx,ts,tsx}",
  "output_mode": "files_with_matches"
}
```

## Returns

Returns search results in the specified format with file paths, line numbers, matched content, and context as requested.