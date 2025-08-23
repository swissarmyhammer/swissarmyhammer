Content-based search with ripgrep integration and intelligent fallback for fast and flexible text searching.

## Overview

The grep tool provides high-performance text searching with automatic engine selection:
- **Primary Engine**: Uses ripgrep when available for optimal performance
- **Fallback Engine**: Falls back to regex-based search if ripgrep is not installed
- **Transparent Operation**: Automatically selects the best available engine
- **Performance Tracking**: Reports which engine was used and execution time

`ripgrep` is used for the matching engine.

## Parameters

- `pattern` (required): Regular expression pattern to search for in file contents
- `path` (optional): File or directory to search in (defaults to current directory)
- `glob` (optional): Glob pattern to filter files (e.g., `*.js`, `**/*.ts`)
- `type` (optional): File type filter (e.g., `js`, `py`, `rust`, `java`, `cpp`)
- `case_insensitive` (optional): Case-insensitive search (default: false)
- `context_lines` (optional): Number of context lines around matches (default: 0)
- `output_mode` (optional): Output format - `content`, `files_with_matches`, or `count` (default: content)

## Use Cases

### Code Analysis
- Finding function definitions, usages, and call sites
- Identifying code patterns and architectural relationships
- Locating specific variable declarations or assignments
- Searching for API usage across a codebase

### Development Workflow
- Finding TODO/FIXME comments and technical debt markers
- Searching for configuration keys and environment variables
- Locating error messages and logging statements
- Identifying deprecated code patterns

### Security and Quality
- Searching for potential security issues or code smells
- Finding hardcoded credentials or sensitive data patterns
- Locating import statements and dependency usage
- Identifying performance anti-patterns

## Examples

### Find function definitions in Rust:
```json
{
  "pattern": "fn\\s+\\w+\\s*\\(",
  "type": "rust",
  "output_mode": "content"
}
```

### Search for TODO comments with context:
```json
{
  "pattern": "TODO|FIXME",
  "case_insensitive": true,
  "context_lines": 2,
  "output_mode": "content"
}
```

### Find TypeScript/JavaScript files importing React:
```json
{
  "pattern": "import.*React",
  "glob": "**/*.{js,jsx,ts,tsx}",
  "output_mode": "files_with_matches"
}
```

### Count occurrences of error handling patterns:
```json
{
  "pattern": "catch\\s*\\(|Result<.*,.*>",
  "case_insensitive": false,
  "output_mode": "count"
}
```

### Search specific directory for configuration keys:
```json
{
  "pattern": "config\\.[A-Z_]+",
  "path": "/path/to/src/config",
  "type": "js",
  "output_mode": "content"
}
```

## Output Format

### Response Structure
All responses include engine information and performance metrics:
- **Engine Used**: Either "ripgrep [version]" or "regex fallback"
- **Execution Time**: Search duration in milliseconds
- **Match Statistics**: Number of matches and files searched

### Output Modes

#### `content` (default)
Returns detailed match information with file paths, line numbers, and matched content:
```
Found 3 matches in 2 files | Engine: ripgrep 13.0.0 | Time: 45ms:

/path/to/file1.rs:15: fn calculate_total() -> Result<i32, Error>
/path/to/file2.rs:8: fn process_data() -> Result<String, ParseError>
/path/to/file2.rs:23: fn validate_input() -> Result<(), ValidationError>
```

#### `files_with_matches`
Returns only file paths containing matches:
```
Files with matches (2) | Engine: ripgrep 13.0.0 | Time: 32ms:
/path/to/file1.rs
/path/to/file2.rs
```

#### `count`
Returns match and file counts:
```
3 matches in 2 files | Engine: ripgrep 13.0.0 | Time: 28ms
```