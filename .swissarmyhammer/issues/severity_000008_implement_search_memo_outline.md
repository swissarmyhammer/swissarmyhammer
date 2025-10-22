# Step 8: Implement Severity for Search, Memoranda, and Outline Errors

**Refer to ideas/severity.md**

## Goal

Implement the `Severity` trait for SearchError, MemorandaError, and OutlineError.

## Context

These crates handle semantic search, memo management, and code outline generation. Errors range from index corruption (critical) to query parsing issues (error).

## Tasks

### 1. Add swissarmyhammer-common Dependency

Ensure all three crates depend on swissarmyhammer-common:

```toml
# In swissarmyhammer-search/Cargo.toml
# In swissarmyhammer-memoranda/Cargo.toml
# In swissarmyhammer-outline/Cargo.toml
[dependencies]
swissarmyhammer-common = { path = "../swissarmyhammer-common" }
```

### 2. Implement Severity for SearchError

In `swissarmyhammer-search/src/error.rs`:

```rust
use swissarmyhammer_common::{ErrorSeverity, Severity};

impl Severity for SearchError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: Search index corrupted
            SearchError::IndexCorrupted { .. } => ErrorSeverity::Critical,
            SearchError::DatabaseError { .. } => ErrorSeverity::Critical,
            SearchError::EmbeddingModelFailed { .. } => ErrorSeverity::Critical,
            
            // Error: Search operation failed
            SearchError::IndexingFailed { .. } => ErrorSeverity::Error,
            SearchError::QueryFailed { .. } => ErrorSeverity::Error,
            SearchError::InvalidQuery { .. } => ErrorSeverity::Error,
            
            // Warning: Non-critical search issues
            SearchError::NoResults { .. } => ErrorSeverity::Warning,
            SearchError::PartialResults { .. } => ErrorSeverity::Warning,
        }
    }
}
```

### 3. Implement Severity for MemorandaError

In `swissarmyhammer-memoranda/src/error.rs`:

```rust
use swissarmyhammer_common::{ErrorSeverity, Severity};

impl Severity for MemorandaError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: Memo storage corrupted
            MemorandaError::StorageCorrupted { .. } => ErrorSeverity::Critical,
            MemorandaError::DirectoryNotWritable { .. } => ErrorSeverity::Critical,
            
            // Error: Memo operation failed
            MemorandaError::MemoNotFound { .. } => ErrorSeverity::Error,
            MemorandaError::InvalidFormat { .. } => ErrorSeverity::Error,
            MemorandaError::WriteFailed { .. } => ErrorSeverity::Error,
            
            // Warning: Non-critical issues
            MemorandaError::EmptyMemo { .. } => ErrorSeverity::Warning,
            MemorandaError::DuplicateTitle { .. } => ErrorSeverity::Warning,
        }
    }
}
```

### 4. Implement Severity for OutlineError

In `swissarmyhammer-outline/src/lib.rs`:

```rust
use swissarmyhammer_common::{ErrorSeverity, Severity};

impl Severity for OutlineError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: Tree-sitter parser failed
            OutlineError::ParserInitializationFailed { .. } => ErrorSeverity::Critical,
            OutlineError::UnsupportedLanguage { .. } => ErrorSeverity::Critical,
            
            // Error: Outline generation failed
            OutlineError::ParseFailed { .. } => ErrorSeverity::Error,
            OutlineError::FileReadError { .. } => ErrorSeverity::Error,
            OutlineError::InvalidSyntax { .. } => ErrorSeverity::Error,
            
            // Warning: Non-critical issues
            OutlineError::IncompleteOutline { .. } => ErrorSeverity::Warning,
            OutlineError::MissingSymbols { .. } => ErrorSeverity::Warning,
        }
    }
}
```

### 5. Add Tests for Each Implementation

```rust
#[cfg(test)]
mod severity_tests {
    use super::*;
    use swissarmyhammer_common::{ErrorSeverity, Severity};

    #[test]
    fn test_search_error_severity() {
        let error = SearchError::IndexCorrupted { /* fields */ };
        assert_eq!(error.severity(), ErrorSeverity::Critical);
        
        let error = SearchError::QueryFailed { /* fields */ };
        assert_eq!(error.severity(), ErrorSeverity::Error);
        
        let error = SearchError::NoResults { /* fields */ };
        assert_eq!(error.severity(), ErrorSeverity::Warning);
    }

    // Similar tests for MemorandaError and OutlineError
}
```

## Severity Guidelines

### Search Errors
**Critical**: Index corruption, database errors, embedding model failures
**Error**: Indexing/query failures, invalid queries
**Warning**: No results, partial results

### Memoranda Errors
**Critical**: Storage corruption, directory not writable
**Error**: Memo not found, write failures
**Warning**: Empty memos, duplicate titles

### Outline Errors
**Critical**: Parser initialization failed, unsupported language
**Error**: Parse failures, file read errors
**Warning**: Incomplete outlines, missing symbols

## Acceptance Criteria

- [ ] SearchError implements Severity trait
- [ ] MemorandaError implements Severity trait
- [ ] OutlineError implements Severity trait
- [ ] Unit tests for each implementation
- [ ] Tests pass for all three crates
- [ ] Code compiles for all three crates
- [ ] Clippy clean for all three crates

## Files to Modify

- `swissarmyhammer-search/Cargo.toml` + `src/error.rs`
- `swissarmyhammer-memoranda/Cargo.toml` + `src/error.rs`
- `swissarmyhammer-outline/Cargo.toml` + `src/lib.rs` (OutlineError location)

## Estimated Changes

~100 lines of code (3 implementations + tests)

## Next Step

Step 9: Implement Severity for templating/agent-executor errors
