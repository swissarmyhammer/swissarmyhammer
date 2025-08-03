# Outline Tool Specification

## Overview

The outline tool provides a structured overview of code definitions across multiple programming languages, expanding beyond top-level symbols to include nested structures. Inspired by [Cline's listCodeDefinitionNamesTool](https://github.com/cline/cline/blob/main/src/core/tools/listCodeDefinitionNamesTool.ts), this tool uses Tree-sitter to parse source code and generate a hierarchical YAML representation of code structure.

## Purpose

- Generate comprehensive code outlines for understanding codebase structure
- Support multiple programming languages through Tree-sitter parsing
- Provide signatures, documentation, and location information
- Maintain file system hierarchy in the output structure
- Enable quick navigation and code comprehension

## Supported Languages

- Rust (.rs)
- TypeScript (.ts)
- JavaScript (.js)
- Dart (.dart)
- Python (.py)

## Parameters

- `patterns` (required): Array of glob patterns to specify which files to parse
  - Examples: `["src/**/*.ts", "lib/**/*.rs"]`
  - Honors `.gitignore` patterns automatically
- `output_format` (optional): Output format, defaults to "yaml"
  - Currently only "yaml" is supported

## Implementation Details

### Tree-sitter Integration

Uses Tree-sitter parsers to extract:
- Type definitions (classes, interfaces, structs, enums)
- Function and method definitions
- Member variables and properties
- Documentation comments
- Source line numbers
- Function signatures and type information

### Output Structure

The tool generates a nested YAML structure that mirrors the file system hierarchy:

```yaml
src:
  utils:
    math.ts:
      children:
        - name: "Calculator"
          kind: "class"
          line: 3
          children:
            - name: "result"
              kind: "property"
              type: "number"
              line: 5
            - name: "add"
              kind: "method"
              signature: "(a: number, b: number) => number"
              line: 8
              doc: "Adds two numbers and returns the result."
        - name: "Operation"
          kind: "enum"
          line: 15
        - name: "multiply"
          kind: "function"
          signature: "(a: number, b: number) => number"
          line: 22
          doc: "Multiplies two numbers."
    string.ts:
      children:
        - name: "StringUtils"
          kind: "class"
          line: 2
          children:
            - name: "capitalize"
              kind: "method"
              signature: "(s: string) => string"
              line: 4
              doc: "Capitalizes the first letter of a string."
```

### Node Properties

Each code element includes:

- `name`: The identifier name
- `kind`: The type of definition (class, function, method, property, etc.)
- `line`: Source line number where the definition starts
- `signature` (optional): Function/method signature with types
- `type` (optional): Type information for properties/variables
- `doc` (optional): Documentation comment text
- `children` (optional): Nested definitions for classes, modules, etc.

### Language-Specific Handling

#### Rust
- Structs, enums, traits, impls
- Functions, methods, associated functions
- Module structures
- Pub visibility indicators
- Rustdoc comments (`///`, `//!`)

#### TypeScript/JavaScript
- Classes, interfaces, types
- Functions, methods, arrow functions
- Properties, getters, setters
- Export/import declarations
- JSDoc comments (`/** */`)

#### Dart
- Classes, abstract classes, mixins
- Functions, methods, constructors
- Properties, fields
- Library and part declarations
- Dartdoc comments (`///`)

#### Python
- Classes, functions, methods
- Properties, class variables
- Module-level definitions
- Decorators
- Docstrings

### Gitignore Integration

The tool automatically respects `.gitignore` patterns to avoid parsing:
- Generated files
- Dependencies (node_modules, target, etc.)
- Build artifacts
- Temporary files

### Error Handling

- Skip files that fail to parse with Tree-sitter
- Log parsing errors for debugging
- Continue processing remaining files
- Report summary of processed vs skipped files

## Usage Examples

### Basic Usage
```json
{
  "patterns": ["src/**/*.ts", "lib/**/*.rs"]
}
```

### Multiple Languages
```json
{
  "patterns": ["**/*.{ts,js,rs,dart,py}"]
}
```

### Specific Directories
```json
{
  "patterns": ["core/**/*.rs", "ui/**/*.dart", "api/**/*.ts"]
}
```

## Output Format

Returns structured YAML representing the parsed code hierarchy, maintaining file system structure while providing detailed symbol information for navigation and understanding.