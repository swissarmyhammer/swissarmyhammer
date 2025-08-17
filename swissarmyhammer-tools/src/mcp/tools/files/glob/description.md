Fast file pattern matching with advanced filtering and sorting.

## Parameters

- `pattern` (required): Glob pattern to match files (e.g., `**/*.js`, `src/**/*.ts`)
- `path` (optional): Directory to search within
- `case_sensitive` (optional): Case-sensitive matching (default: false)
- `respect_git_ignore` (optional): Honor .gitignore patterns (default: true)

## Functionality

- Supports standard glob patterns with wildcards
- Returns file paths sorted by modification time (recent first)
- Searches across multiple workspace directories
- Respects git ignore patterns and workspace boundaries  
- Provides fast pattern matching for large codebases

## Use Cases

- Finding files by name patterns
- Locating specific file types
- Discovering recently modified files
- Building file lists for batch operations

## Examples

Find all JavaScript files:
```json
{
  "pattern": "**/*.js"
}
```

Find TypeScript files in src directory:
```json
{
  "pattern": "**/*.ts",
  "path": "/home/user/project/src"
}
```

Case-sensitive search for README files:
```json
{
  "pattern": "**/README*", 
  "case_sensitive": true,
  "respect_git_ignore": false
}
```

## Returns

Returns a list of matching file paths sorted by modification time (most recent first), along with metadata about each file.