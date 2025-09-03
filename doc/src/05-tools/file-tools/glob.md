# files_glob

Fast file pattern matching with advanced filtering, sorting, and comprehensive .gitignore support.

## Parameters

- `pattern` (required): Glob pattern to match files (e.g., `**/*.js`, `src/**/*.ts`)
- `path` (optional): Directory to search within (defaults to current directory)
- `case_sensitive` (optional): Case-sensitive matching (default: false)
- `respect_git_ignore` (optional): Honor .gitignore patterns (default: true)

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

## CLI Usage

```bash
# Find all Rust files
sah files glob --pattern "**/*.rs"

# Find files in specific directory
sah files glob --pattern "**/*.py" --path ./src

# Case-sensitive search
sah files glob --pattern "**/README*" --case-sensitive

# Ignore gitignore files
sah files glob --pattern "**/*" --no-respect-git-ignore
```

## Use Cases

- **Code Discovery**: Finding files by type or name patterns across large codebases
- **Build Systems**: Collecting source files for compilation or processing
- **Project Analysis**: Identifying recently modified files for review
- **File Organization**: Locating files for cleanup or reorganization
- **Development Workflows**: Building file lists for batch operations

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

## Performance Features

- **Fast Matching**: Uses optimized glob pattern matching
- **Gitignore Integration**: Respects .gitignore, .git/info/exclude, and global excludes
- **Sorted Results**: Results sorted by modification time for relevance
- **Memory Efficient**: Handles large directory trees efficiently