# Step 8: Implement Severity for Search, Memoranda, and Outline Errors

**Refer to ideas/severity.md**

## Goal

Implement the `Severity` trait for SearchError, MemorandaError, and OutlineError.

## Context

These crates handle semantic search, memo management, and code outline generation. Errors range from index corruption (critical) to query parsing issues (error).

## Proposed Solution

After reviewing the actual error types in each crate, here's my implementation plan:

### SearchError Severity Classification

Based on swissarmyhammer-search/src/error.rs:

**Critical** (system cannot continue):
- `Database(String)` - Database corruption/unavailable
- `VectorStorage` - Vector operations failed, search index unusable
- `OnnxRuntime(ort::Error)` - ML model failed, embeddings unavailable

**Error** (operation failed but system continues):
- `Storage(String)` - Storage operation failed
- `Embedding(String)` - Embedding generation failed for specific operation
- `TreeSitter(String)` - Parsing failed for specific file
- `Config(String)` - Invalid configuration
- `Serialization(serde_json::Error)` - Serialization failed
- `Index(String)` - Index operation failed
- `SearchOperation` - Search failed with context
- `Search(String)` - Generic search error
- `Semantic(String)` - Semantic search error

**Warning** (non-critical issues):
- `FileSystem(io::Error)` - File not found or inaccessible (specific files)
- `Io(io::Error)` - IO error for non-critical operations

### MemorandaError Severity Classification

Based on swissarmyhammer-memoranda/src/error.rs:

**Critical** (system cannot continue):
- `Storage(String)` - Storage system failed

**Error** (operation failed but system continues):
- `MemoNotFound { title }` - Specific memo not found
- `InvalidTitle(String)` - Invalid title format
- `Serialization(String)` - Serialization failed
- `InvalidOperation(String)` - Invalid operation attempted
- `Io(io::Error)` - IO error

### OutlineError Severity Classification

Based on swissarmyhammer-outline/src/lib.rs:

**Critical** (system cannot continue):
- None (outline generation failures are not system-level)

**Error** (operation failed but system continues):
- `FileSystem(io::Error)` - File system operation failed
- `InvalidGlobPattern { pattern, message }` - Invalid glob pattern
- `FileDiscovery(String)` - File discovery failed
- `LanguageDetection(String)` - Language detection failed
- `TreeSitter(String)` - TreeSitter parsing failed
- `Generation(String)` - Generic outline generation error

**Warning** (non-critical issues):
- None (all outline errors prevent operation completion)

### Implementation Steps

1. Write failing tests for SearchError severity
2. Implement Severity trait for SearchError
3. Write failing tests for MemorandaError severity
4. Implement Severity trait for MemorandaError
5. Write failing tests for OutlineError severity
6. Implement Severity trait for OutlineError
7. Verify compilation and tests
8. Run clippy to ensure clean code

## Tasks

### 1. Add swissarmyhammer-common Dependency

✅ All three crates already have swissarmyhammer-common dependency

### 2. Implement Severity for SearchError

In `swissarmyhammer-search/src/error.rs`:
- Add use statement for Severity trait
- Implement severity() method with match on all variants
- Add comprehensive tests

### 3. Implement Severity for MemorandaError

In `swissarmyhammer-memoranda/src/error.rs`:
- Add use statement for Severity trait
- Implement severity() method with match on all variants
- Add comprehensive tests

### 4. Implement Severity for OutlineError

In `swissarmyhammer-outline/src/lib.rs`:
- Add use statement for Severity trait
- Implement severity() method with match on all variants
- Add comprehensive tests

## Acceptance Criteria

- [ ] SearchError implements Severity trait
- [ ] MemorandaError implements Severity trait
- [ ] OutlineError implements Severity trait
- [ ] Unit tests for each implementation
- [ ] Tests pass for all three crates
- [ ] Code compiles for all three crates
- [ ] Clippy clean for all three crates

## Files to Modify

- `swissarmyhammer-search/src/error.rs`
- `swissarmyhammer-memoranda/src/error.rs`
- `swissarmyhammer-outline/src/lib.rs`

## Estimated Changes

~150 lines of code (3 implementations + tests)

## Next Step

Step 9: Implement Severity for templating/agent-executor errors



## Code Review Completion Notes

### Issue Fixed

Fixed clippy warning in `swissarmyhammer-outline/src/lib.rs`:
- **Issue**: Test module was placed before pub use statements (line 95)
- **Fix**: Moved test module to end of file after all pub use statements
- **Result**: Clippy now reports no warnings

### Verification

- Build: ✅ Successful (`cargo build --lib -p swissarmyhammer-outline`)
- Clippy: ✅ No warnings (`cargo clippy --lib -p swissarmyhammer-outline`)
- Tests: ✅ All 16 tests pass (`cargo nextest run -p swissarmyhammer-outline`)

### File Modified

`swissarmyhammer-outline/src/lib.rs:94-137` - Moved test module from line 94 to line 107 (after pub use statements)

All code review action items have been completed successfully.
