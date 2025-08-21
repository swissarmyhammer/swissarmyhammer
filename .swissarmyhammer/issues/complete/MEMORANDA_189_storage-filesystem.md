# Implement Memoranda Storage Filesystem Backend

## Overview
Create the filesystem storage backend for memoranda, following the existing patterns from the issues module. This will handle CRUD operations for memos stored in `./.swissarmyhammer/memos/`.

## Tasks

### 1. Create Storage Trait and Implementation
- Create `swissarmyhammer/src/memoranda/storage.rs` following `issues/mod.rs` patterns
- Implement `MemoStorage` trait with async methods:
  - `create_memo(title: String, content: String) -> Result<Memo>`
  - `get_memo(id: &MemoId) -> Result<Memo>`
  - `update_memo(id: &MemoId, content: String) -> Result<Memo>`
  - `delete_memo(id: &MemoId) -> Result<()>`
  - `list_memos() -> Result<Vec<Memo>>`
  - `search_memos(query: &str) -> Result<Vec<Memo>>`

### 2. Directory Management
- Ensure `.swissarmyhammer/memos/` directory is created on first use
- Follow patterns from existing directory utilities
- Proper error handling for permission issues

### 3. File Storage Format
- Store each memo as individual JSON file: `{memo_id}.json`
- Use serde for serialization/deserialization
- Handle file corruption and recovery

### 4. Error Integration
- Add `MemoStorageError` variants to main error enum:
  - `MemoNotFound(MemoId)`
  - `MemoStorageIo(std::io::Error)`
  - `MemoSerialization(serde_json::Error)`

## Implementation Notes
- Follow existing `IssueStorage` patterns exactly
- Use ULID-based filenames for natural sorting
- Async/await throughout for consistency
- Comprehensive error handling

## Acceptance Criteria
- [ ] MemoStorage trait implemented with all CRUD operations
- [ ] Directory creation working with proper permissions
- [ ] JSON file storage working reliably
- [ ] Error types integrated with main error system
- [ ] Basic search functionality (exact string matching)
- [ ] Unit tests covering all storage operations

## Proposed Solution

Based on analysis of the existing codebase patterns, I will implement the memoranda storage backend with the following approach:

### 1. Storage Trait Design
- Create `MemoStorage` trait following `IssueStorage` patterns
- All methods will be async and return `Result<T>` using the main error type
- Follow the exact same API pattern as `IssueStorage` but adapted for memos

### 2. Filesystem Implementation Structure
```rust
pub struct FileSystemMemoStorage {
    state: MemoState,
    creation_lock: Mutex<()>, // Thread-safe memo creation like issues
}

pub struct MemoState {
    memos_dir: PathBuf, // `.swissarmyhammer/memos/`
}
```

### 3. Storage Strategy
- Each memo stored as `{ulid}.json` in `.swissarmyhammer/memos/`
- JSON serialization using existing `Memo` struct
- Use ULID from `MemoId` for natural sorting 
- Directory auto-creation following existing patterns

### 4. Error Handling Integration
- The main error enum already has memo-related variants:
  - `MemoNotFound(String)`
  - `InvalidMemoId(String)`
  - `MemoAlreadyExists(String)` 
  - `MemoValidationFailed(String)`
- Will use existing error variants, no new variants needed
- JSON serialization errors will use existing `Json(serde_json::Error)`

### 5. Implementation Steps
1. **Create storage.rs module** - Mirror issues filesystem structure
2. **Implement MemoStorage trait** - Same async pattern as `IssueStorage` 
3. **Implement FileSystemMemoStorage** - Following existing patterns
4. **Add directory management** - Auto-create `.swissarmyhammer/memos/` 
5. **Implement CRUD operations** - JSON read/write with proper error handling
6. **Add search functionality** - Simple string matching in title/content
7. **Write comprehensive tests** - Following existing test patterns

### 6. Key Implementation Details
- Use `tokio::fs` for async operations (following issues module pattern)
- Implement proper locking during creation (using existing `creation_lock: Mutex<()>` pattern)
- Handle concurrent access safely
- Proper error propagation using `?` operator
- Follow existing coding standards (no comments unless functional, proper error handling)