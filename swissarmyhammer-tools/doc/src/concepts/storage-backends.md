# Storage Backends

SwissArmyHammer Tools uses abstracted storage interfaces for different data types, enabling flexible persistence strategies while maintaining a consistent API.

## Storage Architecture

All storage backends implement trait interfaces, allowing for different implementations (filesystem, database, cloud, etc.) without changing tool code.

## Issue Storage

### Interface
```rust
pub trait IssueStorage: Send + Sync {
    async fn create(&self, issue: &Issue) -> Result<()>;
    async fn list(&self) -> Result<Vec<IssueSummary>>;
    async fn get(&self, name: &str) -> Result<Issue>;
    async fn update(&self, name: &str, content: &str) -> Result<()>;
    async fn complete(&self, name: &str) -> Result<()>;
}
```

### Implementation
- **Type**: `FileSystemIssueStorage`
- **Location**: `.swissarmyhammer/issues/`
- **Format**: Markdown files with YAML frontmatter
- **Features**:
  - Git-friendly (plain text)
  - Human-readable
  - Version-controlled
  - Branch-specific organization

### Directory Structure
```
.swissarmyhammer/
└── issues/
    ├── FEATURE_001_user-auth.md
    ├── BUG_002_memory-leak.md
    └── complete/
        └── FEATURE_000_setup.md
```

## Memo Storage

### Interface
```rust
pub trait MemoStorage: Send + Sync {
    async fn create(&self, title: &str, content: &str) -> Result<Memo>;
    async fn get(&self, title: &str) -> Result<Memo>;
    async fn list(&self) -> Result<Vec<MemoSummary>>;
}
```

### Implementation
- **Type**: `MarkdownMemoStorage`
- **Location**: `.swissarmyhammer/memos/`
- **Format**: Markdown files with metadata
- **Features**:
  - ULID-based identifiers
  - Automatic timestamps
  - Full-text search
  - Context aggregation

## Todo Storage

### Format
YAML file containing todo items:

```yaml
todos:
  - id: "01K97G0ZJE3QE1FMYMZ20ZBGRE"
    task: "Implement feature"
    context: "Additional details"
    done: false
```

### Implementation
- **Location**: `.swissarmyhammer/todo/todo.yaml`
- **Features**:
  - Ephemeral (session-based)
  - Auto-cleanup when complete
  - ULID identifiers
  - Simple structure

## Search Index

### Implementation
- **Type**: SQLite database
- **Location**: `.swissarmyhammer/search.db`
- **Schema**:
  - Files table (paths, languages, timestamps)
  - Chunks table (code segments, embeddings)
  - Metadata table (index statistics)

### Features
- Vector embeddings for semantic similarity
- Tree-sitter parsing for code structure
- Fast similarity queries
- Incremental indexing

## Next Steps

- [MCP Tools Reference](../tools/overview.md) - Tools that use storage
- [File Operations](../tools/file-operations.md) - File tool details
- [Issue Management](../tools/issue-management.md) - Issue workflow
