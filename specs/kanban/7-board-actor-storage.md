# Board Actor and Tag Storage Cleanup

## Status: Partially Fixed

## Problem

Actors and tags should be stored as separate files in `.kanban/actors/*.json` and `.kanban/tags/*.json`, but the `Board` struct still has `actors: Vec<Actor>` and `tags: Vec<Tag>` fields that should not be used.

This creates confusion and maintenance issues:
- Board file still serializes empty actors and tags arrays
- Methods like `board.find_actor()` work on stale data
- Unclear which is the source of truth

## Current State

**Board struct:**
```rust
pub struct Board {
    pub name: String,
    pub description: Option<String>,
    pub columns: Vec<Column>,
    pub swimlanes: Vec<Swimlane>,
    pub actors: Vec<Actor>,  // <-- STALE, should not be used
    pub tags: Vec<Tag>,      // <-- STALE, should not be used
}
```

**Desired storage:**
```
.kanban/
├── board.json         # Should have empty actors and tags arrays
├── actors/
│   ├── alice.json     # Actual actor storage
│   └── assistant.json
├── tags/
│   ├── bug.json       # Actual tag storage
│   └── feature.json
```

**Context methods:**
```rust
// These are the new source of truth for actors
ctx.read_actor(&id)
ctx.list_actor_ids()
ctx.read_all_actors()

// These should be the source of truth for tags
ctx.read_tag(&id)
ctx.list_tag_ids()
ctx.read_all_tags()
```

**Board methods that are now broken:**
```rust
impl Board {
    pub fn find_actor(&self, id: &ActorId) -> Option<&Actor> {
        self.actors.iter().find(|a| a.id() == id)  // Always returns None!
    }

    pub fn find_tag(&self, id: &TagId) -> Option<&Tag> {
        self.tags.iter().find(|t| t.id() == id)  // Will be stale!
    }
}
```

## Required Changes

### 1. Remove actors and tags fields from Board

```rust
pub struct Board {
    pub name: String,
    pub description: Option<String>,
    pub columns: Vec<Column>,
    pub swimlanes: Vec<Swimlane>,
    // pub actors: Vec<Actor>,  // REMOVED
    // pub tags: Vec<Tag>,      // REMOVED
}
```

### 2. Remove Board::find_actor and Board::find_tag methods

Since actors and tags aren't in the board anymore, these methods are misleading:

```rust
impl Board {
    // DELETE THIS:
    // pub fn find_actor(&self, id: &ActorId) -> Option<&Actor>
    // pub fn find_tag(&self, id: &TagId) -> Option<&Tag>
}
```

Instead, code should use `ctx.read_actor(&id)` and `ctx.read_tag(&id)`.

### 3. Update Board::new()

Remove actors and tags initialization:

```rust
impl Board {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            columns: Self::default_columns(),
            swimlanes: Vec::new(),
            // actors: Vec::new(),  // REMOVED
            // tags: Vec::new(),    // REMOVED
        }
    }
}
```

### 4. Implement tag storage operations

Add context methods for tag storage similar to actor storage:

```rust
impl BoardContext {
    pub async fn create_tag(&self, tag: Tag) -> Result<()>
    pub async fn read_tag(&self, id: &TagId) -> Result<Tag>
    pub async fn update_tag(&self, tag: &Tag) -> Result<()>
    pub async fn delete_tag(&self, id: &TagId) -> Result<()>
    pub async fn list_tag_ids(&self) -> Result<Vec<TagId>>
    pub async fn read_all_tags(&self) -> Result<Vec<Tag>>
}
```

### 5. Check for usages

Search for code that accesses `board.actors` or `board.tags` and update to use context methods instead:

```bash
# Find all uses of board.actors and board.tags
rg "board\.actors"
rg "board\.tags"

# Should be replaced with:
ctx.read_all_actors().await?
ctx.read_all_tags().await?
```

## Migration Consideration

**No migration needed** - The system has not shipped yet, so we can make this breaking change without worrying about backwards compatibility. All boards will start fresh with the new file-based storage for actors and tags.

## Why File-Based Storage for Tags?

Moving tags to individual files provides several benefits:

**Consistency:**
- ✅ Same pattern as actors - easier to understand
- ✅ Uniform context API (read_actor, read_tag, etc.)
- ✅ Consistent storage structure

**Version Control:**
- ✅ Better git diffs - see exact tag changes
- ✅ Can track tag history separately
- ✅ Easier to resolve merge conflicts

**Flexibility:**
- ✅ Can add tag-specific metadata without bloating board.json
- ✅ Tags can be created/updated independently
- ✅ Better separation of concerns

## Testing Requirements

- Test board serialization without actors and tags fields
- Test that old boards with actors and tags fields can still be read (backwards compat)
- Test that actor and tag operations work correctly with file-based storage
- Update any tests that rely on `board.actors` or `board.tags`
- Test CRUD operations for tags using context methods

## File Changes

1. `swissarmyhammer-kanban/src/types/board.rs`
   - Remove `actors` and `tags` fields
   - Remove `find_actor()` and `find_tag()` methods
   - Update tests

2. `swissarmyhammer-kanban/src/context.rs`
   - Implement tag storage methods (create, read, update, delete, list)
   - Add `.kanban/tags/` directory support

3. Search and update any code using `board.actors` or `board.tags`

4. Update spec documentation to reflect file-based actor and tag storage

## Priority

**Medium** - This is cleanup/consistency. The system works, but the code is confusing with two different storage locations implied.
