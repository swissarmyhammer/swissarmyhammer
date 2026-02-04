Execute tree-sitter S-expression queries to find AST patterns in code.

Tree-sitter queries use S-expression syntax to match structural patterns in parsed code. This is useful for finding specific code constructs like function definitions, class declarations, or particular syntax patterns.

## Examples

Find all function definitions in Rust:
```json
{
  "query": "(function_item name: (identifier) @name)",
  "language": "rust"
}
```

Find all class definitions in Python:
```json
{
  "query": "(class_definition name: (identifier) @class_name)",
  "language": "python"
}
```

Find function calls in specific files:
```json
{
  "query": "(call_expression function: (identifier) @fn_name)",
  "files": ["src/main.rs", "src/lib.rs"]
}
```

## Query Syntax

Tree-sitter queries use S-expressions:
- `(node_type)` - Match a node type
- `(node_type field: (child_type))` - Match with named fields
- `@name` - Capture a node with a name
- `(node_type (_))` - Match with any child
- `(node_type) @cap (#eq? @cap "text")` - Match with predicates

## Returns

Returns matches with captured nodes, their text content, and file locations.
