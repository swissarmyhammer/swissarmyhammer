# Code Analysis

SwissArmyHammer provides code analysis capabilities through structured outline generation using Tree-sitter parsing.

## Overview

Code analysis helps AI assistants understand code structure by extracting symbols, definitions, and hierarchies from source files. This provides a high-level view without reading entire files.

## Available Tool

### outline_generate

Generate structured code overviews using Tree-sitter parsing.

**Parameters:**
- `patterns` (required): Array of glob patterns to match files (e.g., `["**/*.rs"]`)
- `output_format` (optional): Output format - "yaml" or "json" (default: "yaml")

**Example:**
```json
{
  "patterns": ["**/*.rs"]
}
```

**Returns:**
Hierarchical outline with:
- File paths
- Symbols (classes, functions, methods, etc.)
- Line numbers
- Signatures
- Documentation comments

## Supported Languages

- **Rust**: Modules, functions, structs, enums, traits, impls
- **Python**: Classes, functions, methods, decorators
- **TypeScript**: Classes, interfaces, functions, methods, types
- **JavaScript**: Classes, functions, methods
- **Dart**: Classes, functions, methods, constructors

## Output Format

### YAML Format (Default)

```yaml
files:
  - path: src/main.rs
    language: rust
    symbols:
      - kind: function
        name: main
        line: 10
        signature: "fn main()"
        doc: "Main entry point"
      - kind: struct
        name: Config
        line: 20
        signature: "pub struct Config"
        children:
          - kind: method
            name: new
            line: 25
            signature: "pub fn new() -> Self"
```

### JSON Format

```json
{
  "files": [
    {
      "path": "src/main.rs",
      "language": "rust",
      "symbols": [
        {
          "kind": "function",
          "name": "main",
          "line": 10,
          "signature": "fn main()",
          "doc": "Main entry point"
        }
      ]
    }
  ]
}
```

## Symbol Types

### Rust
- `module`: Module definitions
- `function`: Free functions
- `struct`: Struct definitions
- `enum`: Enum definitions
- `trait`: Trait definitions
- `impl`: Implementation blocks
- `method`: Methods within impls
- `type`: Type aliases
- `const`: Constants
- `static`: Static variables

### Python
- `class`: Class definitions
- `function`: Functions
- `method`: Class methods
- `async_function`: Async functions
- `decorator`: Decorators

### TypeScript/JavaScript
- `class`: Class definitions
- `interface`: Interface definitions (TS)
- `type`: Type aliases (TS)
- `function`: Functions
- `method`: Class methods
- `arrow_function`: Arrow functions

### Dart
- `class`: Class definitions
- `function`: Functions
- `method`: Class methods
- `constructor`: Constructors
- `enum`: Enum definitions

## Use Cases

### Understanding New Codebase

Generate overview of entire codebase:

```json
{
  "patterns": ["src/**/*.rs"],
  "output_format": "yaml"
}
```

Review symbols to understand:
- Module structure
- Public APIs
- Class hierarchies
- Available functions

### Finding Entry Points

Locate main functions and entry points:

```json
{
  "patterns": ["src/main.rs", "src/lib.rs"]
}
```

Look for:
- `main` function
- Public module exports
- Initialization functions

### Documenting APIs

Generate API documentation structure:

```json
{
  "patterns": ["src/lib.rs", "src/**/*.rs"],
  "output_format": "json"
}
```

Use outline to:
- Document public APIs
- Generate API reference
- Create usage examples

### Refactoring Planning

Before refactoring, generate outline:

```json
{
  "patterns": ["src/module/**/*.rs"]
}
```

Analyze:
- Current structure
- Dependencies
- Refactoring scope

### Code Review Preparation

Generate outline of changed files:

```json
{
  "patterns": [
    "src/auth.rs",
    "src/middleware.rs"
  ]
}
```

Review:
- New symbols added
- Modified signatures
- Structural changes

## Symbol Information

### Line Numbers

Each symbol includes its line number in the source file, enabling:
- Direct navigation to definitions
- Cross-referencing with file reads
- Precise location tracking

### Signatures

Function and method signatures show:
- Parameter types
- Return types
- Visibility modifiers
- Generic parameters

Example:
```
pub fn process<T: Clone>(data: &T) -> Result<(), Error>
```

### Documentation

Extracted documentation comments:
- Rust: `///` and `//!` comments
- Python: Docstrings
- TypeScript/JavaScript: JSDoc comments
- Dart: `///` comments

### Hierarchies

Symbol nesting shows relationships:
- Methods within classes
- Functions within modules
- Nested types

## Integration Patterns

### With Semantic Search

1. Generate outline: `outline_generate`
2. Identify interesting symbols
3. Search for usage: `search_query`

### With File Operations

1. Generate outline: `outline_generate`
2. Identify file and line
3. Read specific file: `files_read`

### With Issue Creation

1. Generate outline: `outline_generate`
2. Analyze structure
3. Create refactoring issues: `issue_create`

### With Git Changes

1. Get changed files: `git_changes`
2. Generate outline of changed files: `outline_generate`
3. Review structural changes

## Best Practices

### Pattern Selection

1. **Targeted Patterns**: Use specific patterns for relevant files
2. **Exclude Tests**: Often test files don't need outline
3. **Focus on API**: Outline public interfaces first
4. **Iterate**: Start broad, narrow down as needed

### Output Format

1. **YAML for Humans**: More readable in terminal
2. **JSON for Processing**: Better for programmatic use
3. **Consistent Choice**: Stick with one format per workflow

### Performance

1. **Limit Scope**: Don't outline entire monorepo at once
2. **Cache Results**: Outlines change only when code changes
3. **Parallel Processing**: Tool processes files in parallel

### Analysis Workflow

1. **Start High-Level**: Outline main modules first
2. **Drill Down**: Outline specific files as needed
3. **Cross-Reference**: Use with search and file reads
4. **Document Findings**: Save insights in memos

## Limitations

### No Implementation Details

Outlines show structure, not implementation. For implementation:
- Use `files_read` to read full source
- Use `search_query` to find usage
- Use `files_grep` to search content

### No Cross-File References

Outlines don't show:
- Import relationships
- Call graphs
- Type dependencies

Use semantic search for cross-file analysis.

### Symbol Depth

Very deep nesting may be truncated. Focus on top-level structure.

### Generated Code

May include symbols from generated code if present in source tree.

## Performance Considerations

- **Parsing Speed**: Fast tree-sitter parsing
- **File Count**: Scales to thousands of files
- **Output Size**: Large codebases produce large outlines
- **Memory Usage**: Moderate, proportional to codebase size

## Examples

### Rust Project Outline

```json
{
  "patterns": ["src/**/*.rs"]
}
```

Result:
```yaml
files:
  - path: src/main.rs
    language: rust
    symbols:
      - kind: function
        name: main
        line: 5
        signature: "fn main()"
  - path: src/lib.rs
    language: rust
    symbols:
      - kind: module
        name: api
        line: 3
      - kind: function
        name: initialize
        line: 10
        signature: "pub fn initialize() -> Result<()>"
```

### Python Project Outline

```json
{
  "patterns": ["src/**/*.py"],
  "output_format": "json"
}
```

Result:
```json
{
  "files": [
    {
      "path": "src/main.py",
      "language": "python",
      "symbols": [
        {
          "kind": "function",
          "name": "main",
          "line": 5,
          "signature": "def main()"
        },
        {
          "kind": "class",
          "name": "Application",
          "line": 10,
          "children": [
            {
              "kind": "method",
              "name": "__init__",
              "line": 11
            }
          ]
        }
      ]
    }
  ]
}
```

## Troubleshooting

### No Symbols Found

**Issue:** Outline generated but no symbols.

**Solution:**
- Verify language is supported
- Check file syntax is valid
- Ensure files match patterns

### Incomplete Symbols

**Issue:** Some symbols missing.

**Solution:**
- Verify tree-sitter grammar support
- Check for syntax errors in source
- Update to latest version

### Large Output

**Issue:** Output is too large to process.

**Solution:**
- Use more specific patterns
- Outline one module at a time
- Filter results client-side

## Next Steps

- [Semantic Search](./semantic-search.md): Search code semantically
- [File Operations](./file-operations.md): Read and edit files
- [Issue Management](./issue-management.md): Track refactoring work
