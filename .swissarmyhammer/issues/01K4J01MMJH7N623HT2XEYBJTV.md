# Remove memo_update tool and enhance memo_create to allow memo replacement

## Problem

Having both `memo_create` and `memo_update` tools creates unnecessary complexity. The `memo_update` tool requires knowing the memo ID, while users typically think in terms of memo titles.

## Solution

Remove the `memo_update` tool and enhance `memo_create` to handle memo replacement based on title:

- If a memo with the given title doesn't exist: create a new memo
- If a memo with the given title already exists: replace its content entirely

## Example Usage

```json
// First creation
{"title": "Hello", "content": "World"}  // Creates new memo

// Later replacement  
{"title": "Hello", "content": "World Wide"}  // Replaces existing memo content
```

This approach is more intuitive as users work with memorable titles rather than opaque ULIDs.

## Implementation Details

1. **Remove memo_update tool**:
   - Remove `swissarmyhammer-tools/src/mcp/tools/memoranda/update/mod.rs`
   - Remove from MCP tool registry
   - Remove related tests

2. **Enhance memo_create tool**:
   - Check if memo with given title already exists
   - If exists: replace content and update timestamp, keep same ID
   - If not exists: create new memo as before
   - Return appropriate message indicating creation vs. replacement

3. **Response format**:
   - Include whether this was a creation or replacement
   - Return the memo ID (existing for replacement, new for creation)
   - Clear success message

## Files to Modify

- Remove: `swissarmyhammer-tools/src/mcp/tools/memoranda/update/mod.rs`
- Modify: `swissarmyhammer-tools/src/mcp/tools/memoranda/create/mod.rs`
- Update: MCP tool registry
- Update: Tests for memo creation/replacement

## Acceptance Criteria

- [ ] `memo_update` tool completely removed
- [ ] `memo_create` handles both creation and replacement by title
- [ ] Clear response indicating whether memo was created or replaced
- [ ] Memo ID returned in both cases
- [ ] Existing memo functionality preserved
- [ ] Tests cover both creation and replacement scenarios
- [ ] No breaking changes to memo_get, memo_list, etc.