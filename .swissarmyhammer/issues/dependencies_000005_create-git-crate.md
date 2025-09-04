# Create swissarmyhammer-git Crate

Refer to /Users/wballard/github/swissarmyhammer/ideas/dependencies.md

## Goal

Create a dedicated crate for Git operations by extracting git functionality from the main library and scattered MCP tools.

## Tasks

1. Create new crate structure
2. Move git operations from main library
3. Extract git utilities from MCP tools
4. Create clean API for git operations

## Implementation Details

### Crate Structure
```
swissarmyhammer-git/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── operations.rs      # Core git operations
│   ├── repository.rs      # Repository management
│   ├── branches.rs        # Branch operations
│   ├── commits.rs         # Commit operations
│   ├── status.rs          # Git status queries
│   ├── utils.rs           # Git utilities
│   └── error.rs           # Git-specific errors
```

### Core Dependencies
- `git2` - Git operations
- `swissarmyhammer-common` - Common types and utilities
- `async-trait` - Async trait support
- `tokio` - Async runtime

### Key APIs to Extract

#### From `swissarmyhammer/src/git/operations.rs`
```rust
pub struct GitOperations {
    // Move existing implementation
}

impl GitOperations {
    pub fn new() -> Result<Self, GitError>;
    pub fn create_branch(&self, name: &str) -> Result<(), GitError>;
    pub fn checkout_branch(&self, name: &str) -> Result<(), GitError>;
    // ... other operations
}
```

#### Repository Detection
```rust
pub fn find_repository(start_path: &Path) -> Result<Repository, GitError>;
pub fn is_git_repository(path: &Path) -> bool;
```

## Migration Sources
- `swissarmyhammer/src/git/` - All git modules
- Git operations scattered in MCP tools (issues, etc.)
- Git utilities in workflow actions

## Validation

- [ ] All git operations work correctly
- [ ] Tests pass for branch management
- [ ] Repository detection is reliable
- [ ] Error handling is comprehensive
- [ ] API is clean and well-documented

## Mermaid Diagram

```mermaid
graph TD
    A[Create Crate Structure] --> B[Move Core Operations]
    B --> C[Extract Branch Management]
    C --> D[Add Repository Utils]
    D --> E[Implement Error Handling]
    E --> F[Write Integration Tests]
```

This crate will provide a clean, reusable interface for all Git operations across the project.