# Code Outline Generation Tool

Generate structured code overviews using Tree-sitter parsing for comprehensive code analysis and documentation.

Use this tool to understand the hierarchial structure of the code base, or to extract information about single symbols.

## Description

The `outline_generate` tool creates hierarchical outlines of source code files, extracting symbols like classes, functions, methods, and other code constructs. It uses Tree-sitter parsing to provide accurate, language-aware analysis that preserves the semantic structure of your code.

## Parameters

- `patterns` (required): Array of glob patterns to match files against
  - Supports standard glob patterns like `"**/*.rs"`, `"src/**/*.py"`
  - Multiple patterns can be specified to include different file types
  - Examples: `["**/*.rs"]`, `["src/**/*.ts", "lib/**/*.js"]`

- `output_format` (optional): Output format for the outline (default: "yaml")
  - `"yaml"`: Human-readable YAML format (default)
  - `"json"`: Machine-readable JSON format

## Supported Languages

The tool supports multiple programming languages through Tree-sitter parsers:

- **Rust** (.rs): structs, enums, functions, methods, traits, modules
- **Python** (.py): classes, functions, methods, properties, imports
- **TypeScript** (.ts): classes, interfaces, functions, methods, properties, types
- **JavaScript** (.js): classes, functions, methods, properties, modules
- **Dart** (.dart): classes, functions, methods, properties, constructors

Files that cannot be parsed with Tree-sitter are processed as plain text with basic symbol extraction.

## Symbol Types

The tool recognizes and categorizes the following symbol types:

### Core Language Constructs
- `class`: Class definitions
- `interface`: Interface definitions (TypeScript, Java, etc.)
- `struct`: Struct definitions (Rust, C, etc.)
- `enum`: Enumeration definitions
- `trait`: Trait definitions (Rust) or protocols

### Function-Like Constructs
- `function`: Standalone functions
- `method`: Methods within classes or structs
- `constructor`: Constructor functions or methods

### Data Constructs
- `property`: Properties or class fields
- `field`: Struct or record fields
- `variable`: Variable declarations
- `constant`: Constant definitions

### Organizational Constructs
- `module`: Module definitions
- `namespace`: Namespace definitions
- `import`: Import or use statements
- `type_alias`: Type aliases or typedefs

### Generic
- `other`: Other symbol types not covered above

## Output Structure

The tool returns a structured outline with the following format:

```yaml
outline:
  - name: "ClassName"
    kind: "class"
    line: 10
    signature: "class ClassName:"
    doc: "Class documentation"
    type_info: "class"
    children:
      - name: "method_name"
        kind: "method"
        line: 15
        signature: "def method_name(self, param: str) -> str:"
        doc: "Method documentation"
        type_info: "str -> str"
        children: null
files_processed: 5
symbols_found: 23
execution_time_ms: 150
```

### Field Descriptions

- `name`: Symbol name or identifier
- `kind`: Symbol type from the enum above
- `line`: Line number where the symbol is defined
- `signature`: Optional function/method signature or declaration
- `doc`: Optional documentation string or comment
- `type_info`: Optional type information
- `children`: Nested symbols (for classes, modules, etc.)

## Usage Examples

### Basic Rust Project Analysis
```json
{
  "patterns": ["**/*.rs"]
}
```

### Multi-Language Project with JSON Output
```json
{
  "patterns": ["src/**/*.ts", "lib/**/*.js", "**/*.py"],
  "output_format": "json"
}
```

### Specific Directory Analysis
```json
{
  "patterns": ["src/components/**/*.tsx", "src/utils/**/*.ts"]
}
```

### Configuration Files and Scripts
```json
{
  "patterns": ["*.toml", "scripts/**/*.sh", "config/**/*.yaml"]
}
```

## Performance Characteristics

- **Fast Processing**: Tree-sitter parsing provides efficient analysis
- **Memory Efficient**: Streaming processing for large codebases
- **Concurrent**: Files are processed in parallel when possible
- **Scalable**: Handles projects with thousands of files

