---
name: treesitter
description: Code intelligence using tree-sitter. Use this skill when you need semantic code search, AST pattern matching, or duplicate detection across a codebase.
metadata:
  author: "swissarmyhammer"
  version: "1.0"
---

# Tree-sitter Code Intelligence

The `treesitter` tool provides code intelligence powered by tree-sitter parsing and semantic embeddings. It exposes four operations through a single tool.

## Operations

### get status

Check if the index is ready before running queries. Always do this first if you're unsure whether indexing is complete.

```json
{"op": "get status"}
```

Returns: index readiness, file counts, progress percentage.

### search code

Semantic search using embeddings. Finds code that is *similar in meaning*, not just textual matches. Use this when you want to find implementations that do the same thing, even if the code looks different.

```json
{"op": "search code", "query": "fn process_request(req: Request) -> Response"}
```

```json
{"op": "search code", "query": "error handling with retry logic", "top_k": 5, "min_similarity": 0.8}
```

**Parameters:**
- `query` (required): The code or description to search for
- `top_k`: Maximum results (default: 10)
- `min_similarity`: Similarity threshold 0.0-1.0 (default: 0.9)
- `path`: Workspace path (default: current directory)

**When to use:**
- Finding similar implementations across the codebase
- Locating code patterns ("how is X done elsewhere?")
- Finding related functions when refactoring

### query ast

Execute tree-sitter S-expression queries against the parsed AST. Use this for *structural* searches — finding code by its syntax shape, not its meaning.

```json
{"op": "query ast", "query": "(function_item name: (identifier) @name)", "language": "rust"}
```

```json
{"op": "query ast", "query": "(class_definition name: (identifier) @class_name)", "language": "python"}
```

```json
{"op": "query ast", "query": "(call_expression function: (identifier) @fn_name)", "files": ["src/main.rs"]}
```

**Parameters:**
- `query` (required): Tree-sitter S-expression pattern
- `files`: Specific files to search (default: all indexed files)
- `language`: Language filter (e.g., "rust", "python", "javascript")
- `path`: Workspace path (default: current directory)

**When to use:**
- Finding all function/class/struct definitions
- Locating specific syntax patterns (all `unwrap()` calls, all `async fn`, etc.)
- Navigating unfamiliar codebases by structure

### find duplicates

Detect duplicate code clusters using semantic similarity. Finds not just copy-paste duplicates but also code that does the same thing with different variable names or minor variations.

```json
{"op": "find duplicates"}
```

```json
{"op": "find duplicates", "min_similarity": 0.95, "min_chunk_bytes": 150}
```

```json
{"op": "find duplicates", "file": "src/handlers/user.rs"}
```

**Parameters:**
- `min_similarity`: Similarity threshold 0.0-1.0 (default: 0.85)
- `min_chunk_bytes`: Minimum code chunk size in bytes (default: 100)
- `file`: Find duplicates only for chunks in this file
- `path`: Workspace path (default: current directory)

**When to use:**
- Code quality validation after changes
- Finding refactoring opportunities
- DRY analysis before a PR

## Workflow

1. **Check status first** — run `get status` to confirm the index is ready
2. **Choose the right operation:**
   - Know what the code *does*? Use `search code`
   - Know what the code *looks like*? Use `query ast`
   - Looking for redundancy? Use `find duplicates`
3. **Iterate** — narrow results with `min_similarity`, `language`, or `files` filters

## Tree-sitter Query Syntax Reference

S-expressions match AST node structure:

| Pattern | Meaning |
|---------|---------|
| `(node_type)` | Match a node by type |
| `(node_type field: (child_type))` | Match with named field |
| `@name` | Capture a node |
| `(node_type (_))` | Match with any child |
| `(node_type) @cap (#eq? @cap "text")` | Match with predicate |
| `(node_type "literal")` | Match anonymous node (keyword/operator) |

### Common Patterns by Language

**Rust:**
- Functions: `(function_item name: (identifier) @name)`
- Structs: `(struct_item name: (type_identifier) @name)`
- Impls: `(impl_item type: (type_identifier) @type)`
- Use statements: `(use_declaration argument: (scoped_identifier) @path)`

**Python:**
- Functions: `(function_definition name: (identifier) @name)`
- Classes: `(class_definition name: (identifier) @name)`
- Imports: `(import_from_statement module_name: (dotted_name) @module)`

**JavaScript/TypeScript:**
- Functions: `(function_declaration name: (identifier) @name)`
- Arrow functions: `(arrow_function)`
- Classes: `(class_declaration name: (identifier) @name)`
- Exports: `(export_statement)`
