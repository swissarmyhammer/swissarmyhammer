Generate structured code overviews using Tree-sitter parsing.

## Parameters

- `patterns` (required): Array of glob patterns to match files (e.g., `["**/*.rs"]`)
- `output_format` (optional): Output format - "yaml" or "json" (default: "yaml")

## Examples

```json
{
  "patterns": ["**/*.rs"]
}
```

## Returns

Returns hierarchical outline with symbols (classes, functions, methods, etc.), line numbers, signatures, and documentation. Supports Rust, Python, TypeScript, JavaScript, and Dart.
