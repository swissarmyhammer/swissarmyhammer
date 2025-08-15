```
1. ./swissarmyhammer/src/workflow/cache.rs (score: 0.748)
   pub fn is_expired(&self, ttl: Duration) -> bool {
           self.cached_at.elapsed() > ttl
```

search query results need file and line numbers in a clickable

file:line like ./source.rs:23

you'll need to update the table and indexes to have line number for the extracted functions

## Proposed Solution

After analyzing the codebase, I found that:

1. ✅ The database schema already includes `start_line` and `end_line` fields in the `code_chunks` table
2. ✅ The `CodeChunk` struct already captures line numbers (`start_line` and `end_line`)
3. ✅ Function extraction already captures line numbers during parsing

**The only missing piece is updating the search result display to show clickable filepath with line format.**

### Implementation Steps:

1. **Update CLI search display** in `swissarmyhammer-cli/src/search.rs`:
   - Modify the semantic search result display to show `./source.rs:23` format
   - Use the `start_line` from `result.chunk.start_line` to create clickable references

2. **Update the search result formatting**:
   - Instead of just showing file paths, show them in filepath plus line format
   - This will make them clickable in editors like VS Code

3. **Test the changes** with example searches to ensure the format works correctly

The key change will be updating how results are displayed from:
```rust
result.chunk.file_path.display()
```

To:
```rust
format!("{}:{}", result.chunk.file_path.display(), result.chunk.start_line)
```

This will create clickable references that editors can open directly.