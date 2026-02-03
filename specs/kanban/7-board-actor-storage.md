# Board Actor Storage Cleanup

## Status: Partially Fixed

## Problem

Actors are now stored as separate files in `.kanban/actors/*.json`, but the `Board` struct still has an `actors: Vec<Actor>` field that's no longer used.

This creates confusion and maintenance issues:
- Board file still serializes empty actors array
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
    pub actors: Vec<Actor>,  // <-- STALE, not used
    pub tags: Vec<Tag>,
}
```

**Actual storage:**
```
.kanban/
├── board.json         # Has empty actors array
├── actors/
│   ├── alice.json     # Actual actor storage
│   └── assistant.json
```

**Context methods:**
```rust
// These are the new source of truth
ctx.read_actor(&id)
ctx.list_actor_ids()
ctx.read_all_actors()
```

**Board methods that are now broken:**
```rust
impl Board {
    pub fn find_actor(&self, id: &ActorId) -> Option<&Actor> {
        self.actors.iter().find(|a| a.id() == id)  // Always returns None!
    }
}
```

## Required Changes

### 1. Remove actors field from Board

```rust
pub struct Board {
    pub name: String,
    pub description: Option<String>,
    pub columns: Vec<Column>,
    pub swimlanes: Vec<Swimlane>,
    // pub actors: Vec<Actor>,  // REMOVED
    pub tags: Vec<Tag>,
}
```

### 2. Remove Board::find_actor method

Since actors aren't in the board anymore, this method is misleading:

```rust
impl Board {
    // DELETE THIS:
    // pub fn find_actor(&self, id: &ActorId) -> Option<&Actor>
}
```

Instead, code should use `ctx.read_actor(&id)`.

### 3. Update Board::new()

Remove actors initialization:

```rust
impl Board {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            columns: Self::default_columns(),
            swimlanes: Vec::new(),
            // actors: Vec::new(),  // REMOVED
            tags: Vec::new(),
        }
    }
}
```

### 4. Check for usages

Search for code that accesses `board.actors` and update to use context methods instead:

```bash
# Find all uses of board.actors
rg "board\.actors"

# Should be replaced with:
ctx.read_all_actors().await?
```

## Migration Consideration

**Breaking change**: Existing `.kanban/board.json` files have an `actors` array that will be ignored after this change.

**Migration path:**
1. Add a migration command `sah kanban migrate-actors`
2. Reads `board.json` actors array
3. Writes each actor to `.kanban/actors/{id}.json`
4. Removes actors from board.json

**OR** simpler: Since actors are still in board.json, just document that they're deprecated and will be ignored. New actor operations use file-based storage, old boards continue to work.

## Same Issue with Tags?

**Question**: Should tags also be file-based like actors?

Currently tags are stored in `board.json`. Arguments for each approach:

**Tags in board.json (current):**
- ✅ Fewer files
- ✅ Tags are board-level metadata
- ✅ Tags change less frequently than actors

**Tags as separate files:**
- ✅ Consistent with actors pattern
- ✅ Better git diffs
- ✅ Can track tag history separately

**Recommendation**: Keep tags in board for now unless there's a strong reason to move them.

## Testing Requirements

- Test board serialization without actors field
- Test that old boards with actors field can still be read (backwards compat)
- Test that actor operations work correctly with file-based storage
- Update any tests that rely on `board.actors`

## File Changes

1. `swissarmyhammer-kanban/src/types/board.rs`
   - Remove `actors` field
   - Remove `find_actor()` method
   - Update tests

2. Search and update any code using `board.actors`

3. Update spec documentation to reflect file-based actor storage

## Priority

**Medium** - This is cleanup/consistency. The system works, but the code is confusing with two different storage locations implied.
