Fast file pattern matching with .gitignore support.

## Parameters

- `pattern` (required): Glob pattern to match files (e.g., `**/*.js`, `src/**/*.ts`)
- `path` (optional): Directory to search within (defaults to current directory)
- `case_sensitive` (optional): Case-sensitive matching (default: false)
- `respect_git_ignore` (optional): Honor .gitignore patterns (default: true)

## Examples

```json
{
  "pattern": "**/*.rs"
}
```

## Returns

Returns file count and list of matching file paths sorted by modification time.
