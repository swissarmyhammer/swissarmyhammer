Fast file pattern matching with advanced filtering, sorting, and comprehensive .gitignore support.

## Parameters

- `pattern` (required): Glob pattern to match files (e.g., `**/*.js`, `src/**/*.ts`)
- `path` (optional): Directory to search within (defaults to current directory)
- `case_sensitive` (optional): Case-sensitive matching (default: false)
- `respect_git_ignore` (optional): Honor .gitignore patterns and git repository settings (default: true)

## Use Cases

- **Code Discovery**: Finding files by type or name patterns across large codebases
- **Build Systems**: Collecting source files for compilation or processing
- **Project Analysis**: Identifying recently modified files for review
- **File Organization**: Locating files for cleanup or reorganization
- **Development Workflows**: Building file lists for batch operations

## Examples

### Basic File Type Search
Find all JavaScript files:
```json
{
  "pattern": "**/*.js"
}
```

### Directory-Scoped Search
Find TypeScript files in specific directory:
```json
{
  "pattern": "**/*.ts",
  "path": "/home/user/project/src"
}
```

### Case-Sensitive Matching
Case-sensitive search for README files:
```json
{
  "pattern": "**/README*", 
  "case_sensitive": true,
  "respect_git_ignore": false
}
```

### Complex Pattern Matching
Find test files with specific naming:
```json
{
  "pattern": "**/*{test,spec}.{js,ts}",
  "path": "/project/root"
}
```

### Ignore-Aware Search
Search with full gitignore support (default):
```json
{
  "pattern": "**/*",
  "respect_git_ignore": true
}
```

## Response Format

Returns a structured response with:
- **File Count**: Total number of matching files found
- **File List**: Complete paths to matching files
- **Sort Order**: Files sorted by modification time (most recent first)
- **Performance Info**: Includes warnings if result limit reached

Example response:
```
Found 42 files matching pattern '**/*.rs'

/project/src/main.rs
/project/src/lib.rs
/project/src/utils/helper.rs
...
```
