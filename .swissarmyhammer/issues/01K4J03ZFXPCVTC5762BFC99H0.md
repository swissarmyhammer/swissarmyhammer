# Update memo_list tool to remove ID references - IDs should be internal only

## Problem

The `memo_list` tool currently includes memo IDs in its output, which exposes internal implementation details to users. Since the memo system is moving to a title-based interface where users work exclusively with memo titles, IDs should be kept as internal identifiers only.

## Solution

Update `memo_list` to remove ID references from its response:

- Show memo titles and content previews
- Include creation/modification timestamps
- Remove ULID identifiers from user-facing output
- Keep IDs internal for system operations only

## Current vs Desired Output

```json
// Current (problematic)
{
  "memos": [
    {
      "id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      "title": "Meeting Notes", 
      "preview": "Discussed project roadmap...",
      "created": "2024-01-15T10:30:00Z"
    }
  ]
}

// Desired
{
  "memos": [
    {
      "title": "Meeting Notes",
      "preview": "Discussed project roadmap...", 
      "created": "2024-01-15T10:30:00Z",
      "modified": "2024-01-15T10:30:00Z"
    }
  ]
}
```

## Consistency Benefits

This aligns with the title-based memo workflow:
- `memo_create` works with titles
- `memo_get` retrieves by title  
- `memo_list` shows titles (no IDs)
- Users never need to know or handle IDs

## Files to Modify

- `swissarmyhammer-tools/src/mcp/tools/memoranda/list/mod.rs`
- Update response structure to exclude IDs
- Update tests to not expect IDs in output
- Update tool description if needed

## Acceptance Criteria

- [ ] `memo_list` response excludes memo IDs entirely
- [ ] Response includes titles, previews, and timestamps
- [ ] Clean, user-friendly output focused on content
- [ ] Tests updated to not expect IDs
- [ ] Tool description updated if necessary
- [ ] Consistent with title-based memo interface
- [ ] No breaking changes to core memo functionality