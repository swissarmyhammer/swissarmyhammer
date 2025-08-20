# TP_000195: Project Setup and Dependencies

## Goal
Set up project structure and dependencies for semantic search functionality based on turboprop specifications.

## Context
Adding semantic search capabilities to swissarmyhammer that will allow indexing and searching source code files using vector embeddings. This implements functionality from https://github.com/glamp/turboprop.

## Requirements from Specification
- Use mistral.rs for models and embedding
- Use nomic-ai/nomic-embed-code model quantized to FP8
- Use DuckDB for storing and searching vectors
- Store in .swissarmyhammer directory and ensure it's in .gitignore
- Support TreeSitter parsing for: rust, python, typescript, javascript, dart

## Tasks

### 1. Update Cargo.toml Dependencies
Add required dependencies to workspace Cargo.toml:
```toml
# Semantic Search Dependencies
mistralrs = "0.3"
duckdb = "1.1"
tree-sitter = "0.22"
tree-sitter-rust = "0.21"
tree-sitter-python = "0.21" 
tree-sitter-typescript = "0.21"
tree-sitter-javascript = "0.21"
tree-sitter-dart = "0.21"
md5 = "0.7"
```

### 2. Update .gitignore
Ensure .swissarmyhammer directory is properly ignored:
```
# Semantic search database
.swissarmyhammer/
```

### 3. Create Module Structure
Create new module structure in `swissarmyhammer/src/`:
```
src/
├── semantic/
│   ├── mod.rs
│   ├── types.rs           # Core data structures
│   ├── storage.rs         # DuckDB vector storage
│   ├── embedding.rs       # mistral.rs integration
│   ├── parser.rs          # TreeSitter integration
│   ├── indexer.rs         # File indexing logic
│   ├── searcher.rs        # Query/search logic
│   └── utils.rs           # Utilities and helpers
```

### 4. Update CLI Structure
Add new search subcommand structure to `swissarmyhammer-cli/src/cli.rs`:
```rust
/// Semantic search commands
#[command(subcommand)]
Search {
    #[command(subcommand)]
    command: SearchCommands,
},

#[derive(Subcommand, Debug)]
pub enum SearchCommands {
    /// Index files for semantic search
    Index {
        /// Glob pattern for files to index
        #[arg(short, long)]
        glob: String,
        /// Force re-indexing of all files
        #[arg(short, long)]
        force: bool,
    },
    /// Query indexed files semantically
    Query {
        /// Search query
        query: String,
        /// Number of results to return
        #[arg(short, long, default_value = "10")]
        limit: usize,
        /// Output format
        #[arg(short, long, default_value = "table")]
        format: OutputFormat,
    },
}
```

## Acceptance Criteria
- [ ] All dependencies are added to Cargo.toml
- [ ] .gitignore includes .swissarmyhammer directory
- [ ] Module structure is created with placeholder files
- [ ] CLI subcommand structure is implemented
- [ ] Project compiles without errors
- [ ] Basic module exports are working

## References
- [mistral.rs documentation](https://docs.rs/mistralrs/)
- [DuckDB Rust client](https://docs.rs/duckdb/)
- [TreeSitter Rust bindings](https://docs.rs/tree-sitter/)

## Proposed Solution

I will implement this project setup by:

1. **Dependency Updates**: Add all required semantic search dependencies to the workspace Cargo.toml including mistralrs, duckdb, tree-sitter parsers, and md5
2. **Git Configuration**: Update .gitignore to exclude the .swissarmyhammer directory
3. **Module Structure**: Create the semantic module hierarchy with proper exports and placeholder implementations
4. **CLI Integration**: Add SearchCommands enum and integrate it into the main CLI structure
5. **Testing**: Create basic tests to verify module structure and CLI parsing work correctly
6. **Compilation**: Ensure the entire project compiles cleanly with the new dependencies and structure

I'll follow Test Driven Development by writing tests first, then implementing the minimal code to make them pass.

## Next Steps
After completion, proceed to TP_000196_core-types to define the core data structures.