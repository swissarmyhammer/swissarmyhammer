Fast file pattern matching with advanced filtering, sorting, and comprehensive .gitignore support.

## Parameters

- `pattern` (required): Glob pattern to match files (e.g., `**/*.js`, `src/**/*.ts`)
- `path` (optional): Directory to search within (defaults to current directory)
- `case_sensitive` (optional): Case-sensitive matching (default: false)
- `respect_git_ignore` (optional): Honor .gitignore patterns and git repository settings (default: true)

## Enhanced Functionality

### Advanced Pattern Support
- **Standard Glob Patterns**: `*`, `**`, `?`, `[...]` with full wildcard support
- **Recursive Matching**: `**/*.rs` for recursive file type searches
- **Filename Patterns**: `*.txt` for simple filename matching
- **Directory Patterns**: `src/**/*.py` for directory-specific searches
- **Pattern Validation**: Comprehensive validation with helpful error messages

### Git Integration
- **Full .gitignore Support**: Uses the `ignore` crate for proper .gitignore parsing
- **Git Repository Awareness**: Automatically detects and respects git repository boundaries
- **Nested Gitignore**: Handles nested .gitignore files and directory-specific rules
- **Negation Patterns**: Supports `!important.log` style negation patterns
- **Global Git Config**: Respects global git ignore settings

### Performance Optimizations
- **Result Limiting**: Caps results at 10,000 files to prevent memory exhaustion
- **Early Termination**: Stops processing when result limit is reached
- **Efficient Matching**: Optimized pattern matching for different pattern types
- **Smart File Filtering**: Only processes files, skips directories in results

### Security & Validation
- **Workspace Boundaries**: All file paths validated to be within workspace
- **Pattern Validation**: Validates pattern syntax before execution
- **Path Security**: Uses `FilePathValidator` for comprehensive security checks
- **Length Limits**: Protects against extremely long patterns (max 1000 characters)

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

## Performance Notes

- **Large Codebases**: Optimized for repositories with thousands of files
- **Memory Efficient**: Result limiting prevents memory exhaustion
- **Fast Execution**: Typically completes in under 1 second for most patterns
- **Git Integration**: Minimal overhead from .gitignore processing

## Error Handling

- **Pattern Validation**: Clear error messages for invalid glob patterns
- **Path Validation**: Security validation for all file paths
- **Permission Handling**: Graceful handling of permission denied scenarios
- **Limit Warnings**: Informative messages when result limits are reached