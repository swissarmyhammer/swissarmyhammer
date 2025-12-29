# Tree-sitter MCP Tool Specification

## Problem Statement

The LSP-based tool (see `lsp.md`) provides deep semantic understanding but requires:
- External language server processes
- Server startup time (10-30s for large projects)
- Runtime dependencies (node, npm for TypeScript)
- Project indexing before queries work

For many use cases, a faster, lighter-weight approach is preferable. Tree-sitter can provide:
- **Instant** parsing (no server startup)
- **Zero external dependencies** (parsers compiled into binary)
- **Works on partial/broken code** (error-tolerant parsing)
- **Offline operation** (no network, no subprocesses)

The trade-off: tree-sitter is syntactic, not semantic. It can find definitions and references by name matching, but cannot resolve types or follow imports across modules with the same precision as LSP.

## Background

### Tree-sitter

Tree-sitter is a parser generator tool and incremental parsing library. Key properties:
- Produces concrete syntax trees (lossless - can regenerate source)
- Incremental updates as code changes
- Error recovery for malformed code
- Powerful query language for pattern matching
- Used by GitHub for [code navigation](https://github.blog/open-source/introducing-stack-graphs/)

### Code Navigation Approaches

Tree-sitter supports two approaches for code navigation:

**1. Tags-based (Simple)**
Uses `tags.scm` query files to identify:
- `@definition.function`, `@definition.class`, `@definition.method`, etc.
- `@reference.call`, `@reference.class`, etc.
- `@name` captures for the identifier itself

Matching is by name - find all definitions with name X, find all references to name X.

**2. Stack-graphs (Precise)**
More sophisticated approach using a DSL to define name binding rules:
- Handles imports, scopes, and qualified names
- Produces precise definition resolution
- Used by GitHub for "precise" code navigation
- Requires language-specific stack-graph rules

### Doc Comments

Tree-sitter preserves all comments as nodes in the syntax tree:
- Rust: `line_comment` (`//`, `///`, `//!`), `block_comment` (`/* */`, `/** */`)
- TypeScript/JavaScript: `comment` (includes JSDoc `/** */`)

Doc comments can be associated with definitions by querying for adjacent comment nodes.

## Design Decision: Tags-based vs Stack-graphs

**Recommendation: Start with tags-based, design for stack-graphs later**

Rationale:
- Tags-based is simpler to implement and already covers many use cases
- Stack-graphs require language-specific rule files (more upfront work)
- Can add stack-graphs as an optional "precise" mode later
- Most tree-sitter language grammars already include `tags.scm`

## MCP Tool Design

### Tool: `treesitter`

A single MCP tool with an `operation` parameter, consistent with the `lsp` tool pattern.

**Key Difference from LSP**: No server management, no initialization delay. Parsing happens inline.

**Schema:**
```json
{
  "type": "object",
  "properties": {
    "operation": {
      "type": "string",
      "enum": ["definition", "references", "hover", "parse"],
      "description": "The tree-sitter operation to perform"
    },
    "file_path": {
      "type": "string",
      "description": "Absolute path to the file"
    },
    "line": {
      "type": "integer",
      "description": "Line number (1-based). Required for definition, references, hover.",
      "minimum": 1
    },
    "column": {
      "type": "integer",
      "description": "Column number (1-based). Required for definition, references, hover.",
      "minimum": 1
    },
    "scope": {
      "type": "string",
      "enum": ["file", "directory", "project"],
      "description": "Search scope for definition/references (default: project)",
      "default": "project"
    }
  },
  "required": ["operation", "file_path"]
}
```

---

### Operation: `definition`

Find definitions of the symbol at the given position.

**How it works:**
1. Parse the file at `file_path`
2. Find the identifier node at `line:column`
3. Extract the symbol name
4. Based on `scope`:
   - `file`: Search only within `file_path`
   - `directory`: Glob for all parseable files in same directory as `file_path`
   - `project`: Glob for all parseable files from CWD recursively
5. Parse each file with its appropriate language parser
6. Run `tags.scm` queries to find definitions matching the symbol name
7. Return matching definition locations

**File Discovery (for directory/project scope):**
- Glob for ALL extensions we have parsers for (`.rs`, `.ts`, `.js`, `.py`, `.go`, etc.)
- No project type detection - mixed language projects are supported
- Parse each file with its language-appropriate parser
- Respects `.gitignore` patterns
- Results cached with MD5-based invalidation

**Required parameters:** `file_path`, `line`, `column`
**Optional parameters:** `scope`

**Example:**
```json
{
  "operation": "definition",
  "file_path": "/src/main.rs",
  "line": 15,
  "column": 10
}
```

**Response:**
```json
{
  "symbol": "create_agent",
  "definitions": [
    {
      "file_path": "/src/agent/mod.rs",
      "line": 42,
      "column": 8,
      "kind": "function",
      "preview": "pub fn create_agent(config: &AgentConfig) -> Result<Agent> {"
    }
  ],
  "resolution": "name_match",
  "note": "Multiple definitions possible - tree-sitter uses name matching, not semantic resolution"
}
```

**Important**: Unlike LSP, tree-sitter may return multiple definitions if the same name is defined in multiple places. The tool cannot determine which one is the "correct" definition without semantic analysis.

---

### Operation: `references`

Find all references to the symbol at the given position.

**How it works:**
1. Parse the file at `file_path`
2. Find the identifier node at `line:column`
3. Extract the symbol name
4. Based on `scope`:
   - `file`: Search only within `file_path`
   - `directory`: Glob for all parseable files in same directory as `file_path`
   - `project`: Glob for all parseable files from CWD recursively
5. Parse each file with its appropriate language parser
6. Run `tags.scm` queries to find references matching the symbol name
7. Return matching locations

**File Discovery (for directory/project scope):**
- Glob for ALL extensions we have parsers for
- Mixed language projects fully supported
- Respects `.gitignore` patterns

**Required parameters:** `file_path`, `line`, `column`
**Optional parameters:** `scope`

**Example:**
```json
{
  "operation": "references",
  "file_path": "/src/agent/mod.rs",
  "line": 42,
  "column": 8,
  "scope": "project"
}
```

**Response:**
```json
{
  "symbol": "create_agent",
  "references": [
    {
      "file_path": "/src/main.rs",
      "line": 15,
      "column": 10,
      "kind": "call",
      "preview": "    let agent = create_agent(&config)?;"
    },
    {
      "file_path": "/src/tests/agent_test.rs",
      "line": 8,
      "column": 18,
      "kind": "call",
      "preview": "        let agent = create_agent(&test_config)?;"
    }
  ],
  "total_count": 2,
  "resolution": "name_match"
}
```

---

### Operation: `hover`

Get documentation (doc comments) for the symbol at the given position.

**How it works:**
1. Parse the file at `file_path`
2. Find the identifier node at `line:column`
3. Find the definition(s) of that symbol
4. Query for adjacent comment nodes (doc comments)
5. Return the doc comment content and symbol signature

**Required parameters:** `file_path`, `line`, `column`

**Example:**
```json
{
  "operation": "hover",
  "file_path": "/src/main.rs",
  "line": 15,
  "column": 10
}
```

**Response:**
```json
{
  "symbol": "create_agent",
  "signature": "pub fn create_agent(config: &AgentConfig) -> Result<Agent>",
  "documentation": "Creates a new agent from the given configuration.\n\n# Arguments\n* `config` - The agent configuration\n\n# Returns\nA configured Agent instance or an error.",
  "definition_location": {
    "file_path": "/src/agent/mod.rs",
    "line": 42
  },
  "note": "Documentation extracted from doc comments. No type inference available."
}
```

**Limitations:**
- Only doc comments immediately preceding the definition are captured
- No type inference (signature is extracted syntactically)
- If multiple definitions exist, returns docs from the first match

---

### Operation: `parse`

Parse a file and return the syntax tree structure. Useful for debugging and understanding code structure.

**Required parameters:** `file_path`

**Example:**
```json
{
  "operation": "parse",
  "file_path": "/src/main.rs"
}
```

**Response:**
```json
{
  "language": "rust",
  "root_node": "source_file",
  "symbols": [
    {
      "name": "main",
      "kind": "function",
      "line": 5,
      "column": 4
    },
    {
      "name": "Config",
      "kind": "struct",
      "line": 15,
      "column": 4
    }
  ],
  "errors": [],
  "parse_time_ms": 12
}
```

## Architecture

### Component Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    SwissArmyHammer                          │
│  ┌─────────────────────────────────────────────────────────┤
│  │                   MCP Tools Layer                        │
│  │  ┌─────────────────────────────────────────────────────┐│
│  │  │                 treesitter tool                     ││
│  │  │  operation: definition|references|hover|parse       ││
│  │  └────────────────────────┬────────────────────────────┘│
│  │                           │                              │
│  │  ┌────────────────────────▼─────────────────────────┐   │
│  │  │           TreeSitterBridge (in-process)           │   │
│  │  │  - Language detection from file extension         │   │
│  │  │  - Parser selection and caching                   │   │
│  │  │  - Query execution (tags.scm)                     │   │
│  │  │  - MD5-based file cache for parsed symbols        │   │
│  │  │  - Doc comment extraction                         │   │
│  │  └────────────────────────┬─────────────────────────┘   │
│  │                           │                              │
│  │  ┌────────────────────────▼─────────────────────────┐   │
│  │  │         Compiled-in Language Parsers              │   │
│  │  │  ┌─────────┐ ┌────────────┐ ┌─────────┐          │   │
│  │  │  │  Rust   │ │ TypeScript │ │  More   │          │   │
│  │  │  └─────────┘ └────────────┘ └─────────┘          │   │
│  │  └──────────────────────────────────────────────────┘   │
│  │                                                          │
└──┴──────────────────────────────────────────────────────────┘
```

### TreeSitterBridge Core

```rust
/// In-process tree-sitter parsing and querying
pub struct TreeSitterBridge {
    /// Cached parsers per language (reused across parses)
    parsers: HashMap<String, Parser>,

    /// Cached tag queries per language
    tag_queries: HashMap<String, Query>,

    /// Parsed file cache: path -> (md5_hash, cached_symbols)
    file_cache: HashMap<PathBuf, CachedFile>,

    /// Configuration
    config: TreeSitterConfig,
}

pub struct CachedFile {
    /// MD5 hash of file content when parsed
    content_hash: [u8; 16],

    /// Extracted symbols (definitions, references)
    symbols: Vec<Symbol>,

    /// Parsed tree (optional, for hover/signature extraction)
    tree: Option<Tree>,
}

pub struct TreeSitterConfig {
    /// Maximum file size to parse (default: 10MB)
    max_file_size: usize,

    /// Parse timeout in milliseconds (default: 5000)
    parse_timeout_ms: u64,

    /// Maximum cache entries (default: 10000 files)
    max_cache_entries: usize,
}

impl TreeSitterBridge {
    /// Parse a file and return the tree
    pub fn parse(&mut self, path: &Path) -> Result<Tree, Error>;

    /// Find definitions of symbol at position
    pub fn find_definitions(
        &mut self,
        path: &Path,
        line: usize,
        column: usize,
        scope: SearchScope,
    ) -> Result<Vec<Definition>, Error>;

    /// Find references to symbol at position
    pub fn find_references(
        &mut self,
        path: &Path,
        line: usize,
        column: usize,
        scope: SearchScope,
    ) -> Result<Vec<Reference>, Error>;

    /// Get hover info (docs, signature) for symbol
    pub fn hover(
        &mut self,
        path: &Path,
        line: usize,
        column: usize,
    ) -> Result<HoverInfo, Error>;
}

pub enum SearchScope {
    File,           // Current file only
    Directory,      // Same directory as the queried file
    Project,        // All parseable files from CWD recursively
}
```

### Tag Query Structure

Each language has a `tags.scm` file defining how to identify definitions and references:

```scheme
; Example: Rust tags.scm (simplified)

; Function definitions
(function_item
  name: (identifier) @name) @definition.function

; Method definitions
(function_signature_item
  name: (identifier) @name) @definition.method

; Struct definitions
(struct_item
  name: (type_identifier) @name) @definition.class

; Function calls
(call_expression
  function: (identifier) @name) @reference.call

; Method calls
(call_expression
  function: (field_expression
    field: (field_identifier) @name)) @reference.call
```

### Doc Comment Extraction

```scheme
; Query to find doc comments for a function
(
  (line_comment)+ @doc
  .
  (function_item
    name: (identifier) @name)
  (#match? @doc "^///")
)
```

For Rust, doc comments are:
- `///` - outer doc comments (line_comment starting with `///`)
- `//!` - inner doc comments (line_comment starting with `//!`)
- `/** */` - outer block doc comments
- `/*! */` - inner block doc comments

For TypeScript/JavaScript:
- `/** */` - JSDoc comments

## MVP Languages

**All languages with tree-sitter grammars are supported.** Language is auto-detected from file extension.

No feature flags - all parsers are compiled into the binary for simplicity. This results in a larger binary (~50-100MB) but ensures consistent behavior across all environments.

### Supported Languages (compiled-in)

| Language | Parser Crate | Extensions | Doc Comments |
|----------|--------------|------------|--------------|
| Rust | `tree-sitter-rust` | `.rs` | `///`, `//!`, `/** */` |
| TypeScript | `tree-sitter-typescript` | `.ts`, `.tsx`, `.mts`, `.cts` | JSDoc `/** */` |
| JavaScript | `tree-sitter-javascript` | `.js`, `.jsx`, `.mjs`, `.cjs` | JSDoc `/** */` |
| Python | `tree-sitter-python` | `.py` | `"""docstrings"""` |
| Go | `tree-sitter-go` | `.go` | `// comments` |
| Java | `tree-sitter-java` | `.java` | Javadoc `/** */` |
| C | `tree-sitter-c` | `.c`, `.h` | `/** */`, `///` |
| C++ | `tree-sitter-cpp` | `.cpp`, `.cc`, `.cxx`, `.hpp` | `/** */`, `///` |
| C# | `tree-sitter-c-sharp` | `.cs` | `///` XML docs |
| Ruby | `tree-sitter-ruby` | `.rb` | `#` comments, YARD |
| PHP | `tree-sitter-php` | `.php` | PHPDoc `/** */` |
| Swift | `tree-sitter-swift` | `.swift` | `///`, `/** */` |
| Kotlin | `tree-sitter-kotlin` | `.kt`, `.kts` | KDoc `/** */` |
| Scala | `tree-sitter-scala` | `.scala` | Scaladoc `/** */` |
| Lua | `tree-sitter-lua` | `.lua` | `---` LuaDoc |
| Elixir | `tree-sitter-elixir` | `.ex`, `.exs` | `@doc`, `@moduledoc` |
| Haskell | `tree-sitter-haskell` | `.hs` | `-- |`, `{- | -}` |
| OCaml | `tree-sitter-ocaml` | `.ml`, `.mli` | `(** *)` |
| Zig | `tree-sitter-zig` | `.zig` | `///` |
| Bash | `tree-sitter-bash` | `.sh`, `.bash` | `#` comments |
| HTML | `tree-sitter-html` | `.html`, `.htm` | `<!-- -->` |
| CSS | `tree-sitter-css` | `.css` | `/* */` |
| JSON | `tree-sitter-json` | `.json` | N/A |
| YAML | `tree-sitter-yaml` | `.yaml`, `.yml` | `#` comments |
| TOML | `tree-sitter-toml` | `.toml` | `#` comments |
| Markdown | `tree-sitter-markdown` | `.md` | N/A |
| SQL | `tree-sitter-sql` | `.sql` | `--`, `/* */` |

### Language Auto-Detection

Language is determined from file extension using the existing `detect_language()` function in `swissarmyhammer-rules/src/language.rs`. This provides consistent language detection across the codebase.

## Implementation Plan

### Phase 1: Core Infrastructure

1. **Create TreeSitterBridge struct** in existing or new crate
   - Language detection from file extension
   - Parser initialization and caching
   - Basic tree parsing

2. **Compile in ALL language parsers**
   - All languages listed in "Supported Languages" table
   - No feature flags - all parsers always included
   - Accept larger binary (~50-100MB) for simplicity

3. **Implement query loading**
   - Bundle tags.scm files as embedded resources
   - Query compilation and caching

### Phase 2: MCP Tool

4. **Implement `parse` operation**
   - Basic parsing and symbol extraction
   - Error node detection

5. **Implement `definition` operation**
   - Symbol extraction at position
   - Name-based definition search
   - Multi-file search with scope control

6. **Implement `references` operation**
   - Name-based reference search
   - Scope-limited searching

7. **Implement `hover` operation**
   - Adjacent doc comment extraction
   - Signature extraction from definition node

### Phase 3: Integration

8. **File discovery and caching**
   - Glob for all parseable extensions from CWD
   - Respect `.gitignore` patterns
   - Implement MD5-based content cache
   - LRU eviction when cache exceeds max entries

9. **Performance optimization**
   - Parallel file parsing for multi-file searches
   - Cache hit optimization (hash check before read)
   - Batch symbol extraction

### Phase 4: Polish

10. **Error handling**
    - Graceful handling of parse errors
    - Timeout handling for large files
    - Unsupported language handling

11. **Testing**
    - Unit tests for each operation
    - Integration tests on real projects
    - Performance benchmarks

## Error Handling

### Unsupported Language

```json
{
  "error": {
    "code": "TS_UNSUPPORTED_LANGUAGE",
    "message": "No tree-sitter parser available for .xyz files",
    "details": {
      "file_path": "/src/data.xyz",
      "extension": ".xyz",
      "supported_extensions": [".rs", ".ts", ".tsx", ".js", ".jsx"]
    }
  }
}
```

### Parse Error (recoverable)

```json
{
  "warning": {
    "code": "TS_PARSE_ERRORS",
    "message": "File parsed with errors (results may be incomplete)",
    "details": {
      "error_count": 3,
      "errors": [
        {"line": 15, "column": 10, "message": "Expected ';'"}
      ]
    }
  },
  "definitions": [...]
}
```

### Parse Timeout

```json
{
  "error": {
    "code": "TS_PARSE_TIMEOUT",
    "message": "Parsing timed out after 5000ms",
    "details": {
      "file_path": "/src/huge_generated_file.rs",
      "file_size_bytes": 15000000,
      "timeout_ms": 5000,
      "suggestion": "File may be too large for tree-sitter parsing"
    }
  }
}
```

### Symbol Not Found

```json
{
  "error": {
    "code": "TS_SYMBOL_NOT_FOUND",
    "message": "No identifier found at position",
    "details": {
      "file_path": "/src/main.rs",
      "line": 15,
      "column": 10,
      "node_type": "string_literal",
      "suggestion": "Position may be on whitespace or non-identifier"
    }
  }
}
```

## Comparison: Tree-sitter vs LSP

| Aspect | Tree-sitter | LSP |
|--------|-------------|-----|
| Startup time | Instant | 10-30s (server + indexing) |
| External deps | None (compiled in) | Language server binaries |
| Precision | Name-based matching | Semantic resolution |
| Type info | No | Yes |
| Cross-module | Limited | Full |
| Broken code | Works (error recovery) | May fail |
| Memory | ~10MB per language | 200MB+ per server |
| Offline | Yes | Yes (once running) |

**Use tree-sitter when:**
- Speed is critical
- No external dependencies wanted
- Working with partial/broken code
- Simple name-based navigation is sufficient

**Use LSP when:**
- Precision is critical
- Need type information
- Need cross-module import resolution
- Working with valid, buildable code

## Configuration

```yaml
# .swissarmyhammer/config.yaml
treesitter:
  # Maximum file size to parse (bytes)
  max_file_size: 10485760  # 10MB

  # Parse timeout (milliseconds)
  parse_timeout_ms: 5000

  # Disable specific languages
  disabled_languages: []

  # Default search scope
  default_scope: "project"
```

## Testing Strategy

### Unit Tests

1. **Parser tests**
   - Parse valid Rust/TypeScript files
   - Parse files with syntax errors
   - Handle empty files

2. **Query tests**
   - Definition extraction
   - Reference extraction
   - Doc comment extraction

3. **Position mapping**
   - Line/column to byte offset
   - Handle Unicode correctly

### Integration Tests

4. **Real project tests**
   - Run on SwissArmyHammer itself
   - Compare results with manual inspection

5. **Cross-language tests**
   - Mixed Rust/TypeScript project
   - Correct language detection

### Performance Tests

6. **Benchmarks**
   - Parse time for various file sizes
   - Query time for symbol search
   - Memory usage

## Success Criteria

1. **Speed**: Definition lookup < 50ms for single file, < 500ms for project-wide search
2. **Accuracy**: Name-based matching finds all definitions/references with matching names
3. **Robustness**: Handles syntax errors gracefully, continues with partial results
4. **Zero dependencies**: Works offline with no external processes
5. **Memory efficient**: < 50MB additional memory for typical usage

## Future Extensions

### Stack-graphs Integration (Precise Mode)

Add optional "precise" resolution using stack-graphs:
- More accurate cross-module resolution
- Handles imports and qualified names
- Requires language-specific rule files

```json
{
  "operation": "definition",
  "file_path": "/src/main.rs",
  "line": 15,
  "column": 10,
  "mode": "precise"  // Use stack-graphs instead of name matching
}
```

**Important Note (December 2025):** GitHub's [stack-graphs repository](https://github.com/github/stack-graphs) was archived on September 9, 2025 and is no longer maintained. The last release was `tree-sitter-stack-graphs-v0.10.0` (December 2024).

If we want precise navigation in the future, we would need to:
1. Fork `github/stack-graphs` and maintain it ourselves
2. Write/maintain stack-graph rules for each language (Rust, TypeScript, etc.)
3. Accept the maintenance burden of a complex graph-based name resolution system

Given this, the tags-based approach in the MVP is the pragmatic choice. Stack-graphs integration should only be considered if:
- A community fork emerges with active maintenance, OR
- The precision benefits justify the maintenance cost of our own fork

### Additional Languages

All common languages are already included in MVP. Future additions might include:
- Niche/specialized languages as tree-sitter grammars become available
- Domain-specific languages (DSLs)
- New languages as they gain popularity

### Incremental Updates

For editor integration, support incremental tree updates:
- Track file changes
- Update trees incrementally
- Faster re-parsing for small edits

### Semantic Highlighting

Expose tree-sitter's syntax highlighting capabilities:
- Return highlighted code snippets
- Support theme-aware highlighting

## Design Decisions (Confirmed)

1. **Crate structure**: New `swissarmyhammer-treesitter` crate, separate from existing code

2. **Parser compilation**: Include ALL tree-sitter supported languages, no feature flags. Accept larger binary (~50-100MB) for simplicity and consistent behavior.

3. **Language detection**: Auto-detect from file extension, reuse existing `detect_language()` infrastructure

4. **Tool relationship**: Separate `treesitter` tool (LSP tool may not be implemented - tree-sitter is priority)

5. **Parse operation**: Include in MVP for debugging and tooling use cases

6. **File discovery model**:
   - On-demand parsing only (no background indexing or file watching)
   - No project type detection (no Cargo.toml/package.json sniffing)
   - For directory/project scope: glob for ALL extensions we have parsers for
   - Mixed language projects are the norm - parse everything we can
   - Scope values: `file`, `directory`, `project`
   - Project scope starts from CWD

7. **Caching strategy**:
   - In-memory cache of parsed trees and extracted symbols
   - Content-based cache validation using MD5 hash of source file
   - On parse request: compute MD5, check cache, return cached result if hash matches
   - No persistence to disk - cache lives for duration of MCP server session
   - Simple and fast invalidation without file watching

8. **Query bundling**: Embed queries into binary using `include_str!()`. No external query files needed - single binary deployment.

## References

- [Tree-sitter Documentation](https://tree-sitter.github.io/tree-sitter/)
- [Tree-sitter Code Navigation](https://tree-sitter.github.io/tree-sitter/4-code-navigation.html)
- [GitHub Stack Graphs](https://github.blog/open-source/introducing-stack-graphs/)
- [tree-sitter-rust](https://github.com/tree-sitter/tree-sitter-rust)
- [tree-sitter-stack-graphs](https://github.com/github/stack-graphs/tree/main/tree-sitter-stack-graphs)
- [GitHub Semantic Analysis](https://dl.acm.org/doi/fullHtml/10.1145/3487019.3487022)
