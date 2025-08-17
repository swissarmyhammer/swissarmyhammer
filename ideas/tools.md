# File Editing Tools Specification

A comprehensive suite of file manipulation and search tools for AI-assisted development environments.

## Overview

This specification defines five core file editing tools that provide essential file system operations for code analysis, editing, and search functionality. These tools are designed to work together as a cohesive system for file management in development workflows.

## Tool Specifications

### Read Tool

**Purpose**: Read and return file contents from the local filesystem with support for various file types.

**Parameters**:
- `absolute_path` (required): Full absolute path to the file
- `offset` (optional): Starting line number for partial reading
- `limit` (optional): Maximum number of lines to read

**Functionality**:
- Validates file path (must be absolute and within workspace)
- Supports text files, images, PDFs, and other file types
- Enables partial file reading via offset/limit for large files
- Provides error handling for missing or inaccessible files
- Respects workspace boundaries and ignore patterns

**Use Cases**:
- Reading source code files for analysis
- Examining configuration files
- Viewing documentation or README files
- Reading specific sections of large files

### Edit Tool

**Purpose**: Perform precise string replacements in existing files.

**Parameters**:
- `file_path` (required): Absolute path to the file to modify
- `old_string` (required): Exact text to replace
- `new_string` (required): Replacement text
- `replace_all` (optional): Replace all occurrences (default: false)

**Functionality**:
- Performs exact string matching and replacement
- Maintains file encoding and line endings
- Validates that old_string exists and is unique (unless replace_all is true)
- Provides atomic operations (all or nothing replacement)
- Preserves file permissions and metadata

**Use Cases**:
- Modifying specific code sections
- Updating variable names or function signatures
- Fixing bugs with targeted changes
- Refactoring code with precise replacements

### Write Tool

**Purpose**: Create new files or completely overwrite existing files.

**Parameters**:
- `file_path` (required): Absolute path for the new or existing file
- `content` (required): Complete file content to write

**Functionality**:
- Creates new files with specified content
- Overwrites existing files completely
- Creates parent directories if they don't exist
- Sets appropriate file permissions
- Validates file path and content

**Use Cases**:
- Creating new source files
- Generating configuration files
- Writing documentation or README files
- Creating test files or fixtures

### Glob Tool

**Purpose**: Fast file pattern matching with advanced filtering and sorting.

**Parameters**:
- `pattern` (required): Glob pattern to match files (e.g., `**/*.js`, `src/**/*.ts`)
- `path` (optional): Directory to search within
- `case_sensitive` (optional): Case-sensitive matching (default: false)
- `respect_git_ignore` (optional): Honor .gitignore patterns (default: true)

**Functionality**:
- Supports standard glob patterns with wildcards
- Returns file paths sorted by modification time (recent first)
- Searches across multiple workspace directories
- Respects git ignore patterns and workspace boundaries
- Provides fast pattern matching for large codebases

**Use Cases**:
- Finding files by name patterns
- Locating specific file types
- Discovering recently modified files
- Building file lists for batch operations

### Grep Tool

**Purpose**: Content-based search using ripgrep for fast and flexible text searching.

**Parameters**:
- `pattern` (required): Regular expression pattern to search
- `path` (optional): File or directory to search in
- `glob` (optional): Glob pattern to filter files (e.g., `*.js`)
- `type` (optional): File type filter (e.g., `js`, `py`, `rust`)
- `case_insensitive` (optional): Case-insensitive search
- `context_lines` (optional): Number of context lines around matches
- `output_mode` (optional): Output format (`content`, `files_with_matches`, `count`)

**Functionality**:
- Leverages ripgrep for high-performance text search
- Supports full regular expression syntax
- Provides file type and glob filtering
- Returns contextual information around matches
- Handles large codebases efficiently

**Use Cases**:
- Finding function definitions or usages
- Searching for specific code patterns
- Locating configuration values
- Identifying potential issues or code smells

## Integration Patterns

### Tool Composition
These tools are designed to work together:
- Use **Glob** to find relevant files, then **Read** to examine contents
- Use **Grep** to locate specific code, then **Edit** to make changes
- Use **Read** before **Edit** to understand context
- Use **Write** for new files, **Edit** for modifications

### Error Handling
All tools should provide:
- Clear error messages for invalid parameters
- Workspace boundary validation
- File permission checks
- Graceful handling of missing files or directories

### Performance Considerations
- **Glob** and **Grep** are optimized for large codebases
- **Read** supports partial reading for large files
- **Edit** performs atomic operations
- All tools respect workspace boundaries to limit scope

## Implementation Requirements

### Security
- Validate all file paths are within workspace boundaries

### Reliability
- Atomic operations where possible
- Comprehensive error handling and validation
- Consistent behavior across different file types
- Proper handling of encoding and line endings

### Performance
- Efficient pattern matching algorithms
- Minimal memory usage for large files
- Fast search capabilities using ripgrep
- NO CACHING