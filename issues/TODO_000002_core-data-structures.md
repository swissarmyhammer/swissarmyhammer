# Create Core Data Structures and YAML Handling

Refer to ./specification/todo_tool.md

## Overview
Implement the core data structures and YAML serialization/deserialization for todo items following the specification.

## Data Structure Requirements
Based on specification, implement:
```yaml
todo:
  - id: 01K1KQM85501ECE8XJGNZKNJQW
    task: "Implement file read tool"
    context: "Use cline's readTool.ts for inspiration"
    done: true
```

## Tasks
1. Create `TodoItem` struct with proper serde derives:
   - `id`: String (ULID)
   - `task`: String (required)
   - `context`: Option<String> (optional)
   - `done`: bool (default false)

2. Create `TodoList` struct containing Vec<TodoItem>

3. Implement YAML serialization/deserialization using `serde_yaml`

4. Create utility functions for:
   - Loading todo list from file
   - Saving todo list to file  
   - Generating sequential ULIDs
   - File path validation and creation

5. Add proper error handling for file operations

## File Structure
Todo lists stored as `.yaml` files in `./swissarmyhammer/todo/` directory

## Success Criteria
- Data structures compile and serialize correctly
- YAML round-trip works (load -> save -> load)
- ULID generation produces sequential identifiers
- File operations handle missing directories gracefully
- Error types integrate with existing error handling

## Implementation Notes
- Use existing patterns from memoranda for file operations
- Leverage existing ULID utilities if available
- Follow Rust naming conventions and derive patterns
- Add comprehensive unit tests for data structures