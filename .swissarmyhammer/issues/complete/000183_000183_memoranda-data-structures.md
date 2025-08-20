# Implement Core Memoranda Data Structures

## Overview
Create the core data structures and types needed for memoranda functionality, following Rust best practices and swissarmyhammer patterns.

## Tasks

### 1. Create Memoranda Module
- Create `swissarmyhammer/src/memoranda/mod.rs` 
- Set up the module structure and exports
- Add to `swissarmyhammer/src/lib.rs`

### 2. Core Data Types
Implement the following core types:

```rust
// Core memo structure
pub struct Memo {
    pub id: MemoId,
    pub title: String, 
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Strong-typed ID to prevent confusion with other IDs
pub struct MemoId(String); // ULID-based per coding standards

// Request/Response types for MCP integration
pub struct CreateMemoRequest {
    pub title: String,
    pub content: String,
}

pub struct UpdateMemoRequest {
    pub id: MemoId,
    pub content: String,
}

pub struct SearchMemosRequest {
    pub query: String,
}
```

### 3. Error Types
- Create `MemorandaError` enum following existing error patterns
- Implement proper error conversion and display
- Integration with existing error handling

### 4. Serialization Support
- Add serde derives for JSON serialization
- Ensure compatibility with MCP protocol requirements
- Test serialization/deserialization

## Acceptance Criteria
- [ ] All data structures implemented with proper types
- [ ] Serde serialization working correctly
- [ ] Error types integrated with existing error handling
- [ ] Module properly exported from lib.rs
- [ ] No compilation errors

## Implementation Notes
- Follow existing patterns from issues module
- Use ULID for memo IDs per coding standards  
- Keep data structures simple and focused
- Ensure compatibility with async/await patterns

## Proposed Solution

### 1. Module Structure
Create `swissarmyhammer/src/memoranda/mod.rs` following the pattern of the issues module:
- Core data structures (Memo, MemoId, requests/responses)
- Error types integrated with existing SwissArmyHammerError
- Serde derives for JSON serialization
- Export from main lib.rs

### 2. Core Data Types
Following the pattern of IssueNumber for strong typing:
- `MemoId(String)` - ULID-based wrapper to prevent ID confusion
- `Memo` struct with proper DateTime fields using chrono::Utc
- Request/response types for MCP integration
- All types will have serde derives for JSON compatibility

### 3. Error Integration
Add MemorandaError variants to SwissArmyHammerError enum:
- MemoNotFound
- InvalidMemoId  
- MemoDuplicationError
- MemoValidationError

### 4. Implementation Steps
1. Create memoranda module structure
2. Implement core data types with proper validation
3. Add error types to main error enum
4. Write comprehensive tests for serialization/deserialization
5. Export from lib.rs following existing patterns

This follows the exact same patterns as the issues module with strong typing, comprehensive error handling, and proper async support.