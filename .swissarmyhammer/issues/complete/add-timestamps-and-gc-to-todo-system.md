# Add Timestamps and Garbage Collection to Todo System

## Problem

Todo items currently lack timestamps, making it impossible to:
- Track when a todo was created or last updated
- Identify stale or old todos that should be cleaned up
- Implement automatic cleanup of completed todos

Additionally, completed todos accumulate indefinitely, cluttering the todo file.

## Proposed Solution

Add UTC ISO8601 timestamps to todos and implement automatic garbage collection of old completed todos.

## Requirements

### 1. Add Timestamps to Todo Items

Update the `TodoItem` struct in `swissarmyhammer-todo` to include:

```rust
pub struct TodoItem {
    pub id: TodoId,
    pub task: String,
    pub context: Option<String>,
    pub done: bool,
    pub created_at: DateTime<Utc>,      // NEW: UTC timestamp when created
    pub updated_at: DateTime<Utc>,      // NEW: UTC timestamp when last updated
}
```

**Timestamp Format**: Use ISO8601 format with UTC timezone (e.g., `2025-11-12T10:30:00Z`)

**When to Update**:
- `created_at`: Set once when todo is created, never changed
- `updated_at`: Updated when:
  - Todo is created (same as created_at initially)
  - Todo is marked complete
  - Todo content is modified (if we add update functionality)

### 2. Update YAML Serialization

Ensure timestamps are serialized/deserialized correctly in the YAML file:

```yaml
- id: 01K9SK4A6KNMZR0R2GERZYTRT6
  task: "Fix the bug"
  context: "Need to check the validation logic"
  done: false
  created_at: "2025-11-12T10:30:00Z"
  updated_at: "2025-11-12T10:30:00Z"
```

### 3. Implement Garbage Collection

Add automatic cleanup of old completed todos when creating a new todo:

**Trigger**: Run GC when `todo_create` is called  
**Criteria**: Delete todos where:
- `done: true` AND
- `updated_at` is more than 24 hours ago

**Implementation Location**: 
- Add `gc_completed_todos()` method to `TodoStorage`
- Call it from `create_todo_item()` before creating the new todo
- Log how many todos were garbage collected

**Example**:
```rust
impl TodoStorage {
    pub async fn create_todo_item(&self, task: String, context: Option<String>) -> Result<TodoItem> {
        // First, garbage collect old completed todos
        let gc_count = self.gc_completed_todos(Duration::hours(24)).await?;
        if gc_count > 0 {
            tracing::info!("Garbage collected {} completed todos older than 24 hours", gc_count);
        }
        
        // Then create the new todo
        let now = Utc::now();
        let todo = TodoItem {
            id: TodoId::new(),
            task,
            context,
            done: false,
            created_at: now,
            updated_at: now,
        };
        // ... rest of creation logic
    }
    
    async fn gc_completed_todos(&mut self, max_age: Duration) -> Result<usize> {
        // Remove completed todos older than max_age
        // Return count of removed todos
    }
}
```

### 4. Update mark_complete to Update Timestamp

When marking a todo complete:
```rust
pub async fn mark_todo_complete(&mut self, id: &TodoId) -> Result<()> {
    // Find the todo
    let todo = self.find_todo_mut(id)?;
    
    // Update both fields
    todo.done = true;
    todo.updated_at = Utc::now();  // NEW: Update timestamp
    
    // Save changes
    self.save().await
}
```

### 5. Migration Strategy

**Backward Compatibility**:
- When loading existing todos without timestamps, default to:
  - `created_at`: Current time (best guess)
  - `updated_at`: Current time
- Log a warning about missing timestamps
- Consider writing timestamps back on first load

**Testing**:
- Test loading old YAML files without timestamps
- Test that new todos have correct timestamps
- Test GC with various scenarios (no completed todos, some old, some new)

## Benefits

1. **Automatic Cleanup**: No manual intervention needed to clean up old todos
2. **Timestamps for Debugging**: Can see when todos were created/updated
3. **File Size Management**: Keeps todo.yaml small and focused on active work
4. **History Awareness**: Can track how long todos have been pending

## Implementation Notes

### Dependencies
- Already using `chrono` crate for DateTime support
- `serde` already handles DateTime serialization to ISO8601

### Configuration
Consider making GC age configurable:
- Default: 24 hours
- Could add env var: `SAH_TODO_GC_AGE_HOURS`
- Could add to config file

### Edge Cases
- What if GC fails? Log error but don't fail todo creation
- What if clock skew? Use `updated_at` not `created_at` for GC decisions
- What about todos completed seconds ago? Should survive GC (check > 24h, not >= 24h)

### MCP Tool Updates

Update `todo_create` tool response to include timestamp info:
```json
{
  "message": "Created todo item",
  "todo_item": {
    "id": "01XXXXX",
    "task": "Fix bug",
    "context": "...",
    "done": false,
    "created_at": "2025-11-12T10:30:00Z",
    "updated_at": "2025-11-12T10:30:00Z"
  },
  "gc_count": 3  // Number of old completed todos removed
}
```

## Testing Requirements

1. **Unit Tests**:
   - Test timestamp assignment on create
   - Test timestamp update on mark_complete
   - Test GC logic with various ages
   - Test loading old YAML without timestamps

2. **Integration Tests**:
   - Create multiple todos, complete some, wait, create new todo, verify GC
   - Test that todos < 24h old are not GC'd
   - Test that todos > 24h old are GC'd

3. **Migration Tests**:
   - Load existing todo.yaml files
   - Verify backward compatibility

## Related

This supports better hygiene in the todo system used by the review workflow and other automated workflows. Completed todos from previous review cycles will be automatically cleaned up.

# Add Timestamps and Garbage Collection to Todo System

## Problem

Todo items currently lack timestamps, making it impossible to:
- Track when a todo was created or last updated
- Identify stale or old todos that should be cleaned up
- Implement automatic cleanup of completed todos

Additionally, completed todos accumulate indefinitely, cluttering the todo file.

## Proposed Solution

Add UTC ISO8601 timestamps to todos and implement automatic garbage collection of old completed todos.

## Requirements

### 1. Add Timestamps to Todo Items

Update the `TodoItem` struct in `swissarmyhammer-todo` to include:

```rust
pub struct TodoItem {
    pub id: TodoId,
    pub task: String,
    pub context: Option<String>,
    pub done: bool,
    pub created_at: DateTime<Utc>,      // NEW: UTC timestamp when created
    pub updated_at: DateTime<Utc>,      // NEW: UTC timestamp when last updated
}
```

**Timestamp Format**: Use ISO8601 format with UTC timezone (e.g., `2025-11-12T10:30:00Z`)

**When to Update**:
- `created_at`: Set once when todo is created, never changed
- `updated_at`: Updated when:
  - Todo is created (same as created_at initially)
  - Todo is marked complete
  - Todo content is modified (if we add update functionality)

### 2. Update YAML Serialization

Ensure timestamps are serialized/deserialized correctly in the YAML file:

```yaml
- id: 01K9SK4A6KNMZR0R2GERZYTRT6
  task: "Fix the bug"
  context: "Need to check the validation logic"
  done: false
  created_at: "2025-11-12T10:30:00Z"
  updated_at: "2025-11-12T10:30:00Z"
```

### 3. Implement Garbage Collection

Add automatic cleanup of old completed todos when creating a new todo:

**Trigger**: Run GC when `todo_create` is called  
**Criteria**: Delete todos where:
- `done: true` AND
- `updated_at` is more than 24 hours ago

**Implementation Location**: 
- Add `gc_completed_todos()` method to `TodoStorage`
- Call it from `create_todo_item()` before creating the new todo
- Log how many todos were garbage collected

**Example**:
```rust
impl TodoStorage {
    pub async fn create_todo_item(&self, task: String, context: Option<String>) -> Result<TodoItem> {
        // First, garbage collect old completed todos
        let gc_count = self.gc_completed_todos(Duration::hours(24)).await?;
        if gc_count > 0 {
            tracing::info!("Garbage collected {} completed todos older than 24 hours", gc_count);
        }
        
        // Then create the new todo
        let now = Utc::now();
        let todo = TodoItem {
            id: TodoId::new(),
            task,
            context,
            done: false,
            created_at: now,
            updated_at: now,
        };
        // ... rest of creation logic
    }
    
    async fn gc_completed_todos(&mut self, max_age: Duration) -> Result<usize> {
        // Remove completed todos older than max_age
        // Return count of removed todos
    }
}
```

### 4. Update mark_complete to Update Timestamp

When marking a todo complete:
```rust
pub async fn mark_todo_complete(&mut self, id: &TodoId) -> Result<()> {
    // Find the todo
    let todo = self.find_todo_mut(id)?;
    
    // Update both fields
    todo.done = true;
    todo.updated_at = Utc::now();  // NEW: Update timestamp
    
    // Save changes
    self.save().await
}
```

### 5. Migration Strategy

**Backward Compatibility**:
- When loading existing todos without timestamps, default to:
  - `created_at`: Current time (best guess)
  - `updated_at`: Current time
- Log a warning about missing timestamps
- Consider writing timestamps back on first load

**Testing**:
- Test loading old YAML files without timestamps
- Test that new todos have correct timestamps
- Test GC with various scenarios (no completed todos, some old, some new)

## Benefits

1. **Automatic Cleanup**: No manual intervention needed to clean up old todos
2. **Timestamps for Debugging**: Can see when todos were created/updated
3. **File Size Management**: Keeps todo.yaml small and focused on active work
4. **History Awareness**: Can track how long todos have been pending

## Implementation Notes

### Dependencies
- Already using `chrono` crate for DateTime support
- `serde` already handles DateTime serialization to ISO8601

### Configuration
Consider making GC age configurable:
- Default: 24 hours
- Could add env var: `SAH_TODO_GC_AGE_HOURS`
- Could add to config file

### Edge Cases
- What if GC fails? Log error but don't fail todo creation
- What if clock skew? Use `updated_at` not `created_at` for GC decisions
- What about todos completed seconds ago? Should survive GC (check > 24h, not >= 24h)

### MCP Tool Updates

Update `todo_create` tool response to include timestamp info:
```json
{
  "message": "Created todo item",
  "todo_item": {
    "id": "01XXXXX",
    "task": "Fix bug",
    "context": "...",
    "done": false,
    "created_at": "2025-11-12T10:30:00Z",
    "updated_at": "2025-11-12T10:30:00Z"
  },
  "gc_count": 3  // Number of old completed todos removed
}
```

## Testing Requirements

1. **Unit Tests**:
   - Test timestamp assignment on create
   - Test timestamp update on mark_complete
   - Test GC logic with various ages
   - Test loading old YAML without timestamps

2. **Integration Tests**:
   - Create multiple todos, complete some, wait, create new todo, verify GC
   - Test that todos < 24h old are not GC'd
   - Test that todos > 24h old are GC'd

3. **Migration Tests**:
   - Load existing todo.yaml files
   - Verify backward compatibility

## Related

This supports better hygiene in the todo system used by the review workflow and other automated workflows. Completed todos from previous review cycles will be automatically cleaned up.

## Proposed Implementation Plan

I will use Test-Driven Development (TDD) to implement this feature step by step:

### Phase 1: Add Timestamps to TodoItem
1. **Add chrono dependency** to swissarmyhammer-todo/Cargo.toml if not present
2. **Update TodoItem struct** in types.rs to include created_at and updated_at fields
3. **Write failing tests** for:
   - Creating a new todo with timestamps
   - Timestamps being set to current time on creation
   - Timestamps being identical on creation
4. **Update TodoItem::new()** to set both timestamps to Utc::now()
5. **Update TodoList::add_item()** to handle new fields
6. **Run tests** to verify they pass

### Phase 2: Update mark_complete to Update Timestamp
1. **Write failing test** for updated_at changing when marking complete
2. **Update TodoItem::mark_complete()** to set updated_at to Utc::now()
3. **Run tests** to verify timestamp updates work

### Phase 3: Implement Backward Compatibility (Migration)
1. **Write test** for loading old YAML without timestamps
2. **Update serde deserialization** to use default timestamps if missing
   - Add `#[serde(default = "Utc::now")]` or custom deserializer
3. **Add warning log** when missing timestamps are detected
4. **Run tests** to verify old YAML loads correctly

### Phase 4: Implement Garbage Collection
1. **Write failing tests** for gc_completed_todos:
   - No completed todos (gc_count = 0)
   - Some completed todos < 24h old (gc_count = 0)
   - Some completed todos > 24h old (gc_count = N)
   - Mix of old and new completed todos
2. **Add gc_completed_todos() method** to TodoStorage
   - Load current list
   - Filter out completed todos > 24h old
   - Save updated list
   - Return count of removed todos
3. **Update create_todo_item()** to call gc_completed_todos before creating
4. **Add tracing log** for GC activity
5. **Run tests** to verify GC works correctly

### Phase 5: Integration Tests
1. **Write end-to-end test** simulating time passage
   - Create todos
   - Mark some complete
   - Simulate 24h+ passage (mock time or adjust timestamps)
   - Create new todo
   - Verify old completed todos are gone
2. **Run all tests** to ensure full system works

### Phase 6: Update MCP Tool Response
1. **Update todo_create tool** in swissarmyhammer-tools to return gc_count
2. **Update response JSON** to include timestamp fields
3. **Test MCP tool** integration

This plan ensures each step is small, testable, and builds on the previous step. I'll follow TDD principles strictly: write failing test → implement minimal code to pass → refactor if needed → move to next test.
