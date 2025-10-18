# Remove notify_create from CODING_STANDARDS Memo

## Parent Issue
Eliminate notify_create Tool and Replace with Native MCP Notifications (01K7SM38449JYA2KZP4KNKQ05X)

## Summary
Remove instructions to use notify_create tool from the CODING_STANDARDS memo.

## Location
`.swissarmyhammer/memos/CODING_STANDARDS.md`

## Tasks

1. Find the refactoring section that mentions notify_create
2. Remove or update the instruction:
```markdown
- notify_create the user that an issue was created using the notify_create tool
```

3. Replace with guidance about when tools should send MCP progress notifications:
```markdown
### Refactoring

When you are engaged in a large refactoring, you need to work file by file.

- Search and define a list of files you will be changing in this refactoring, create a todo list of these files with todo_create
- Plan out the change to each file, creating a new todo list item with todo_create
  - in each todo, clearly state the goal of the refactoring
  - in each todo, clearly state the change you will be making
  - in each todo, clearly state what tests need to be created
- Work the todo list
- todo_mark_complete it off your todo list once the issue is created

Creating issues rather than just 'going for it' ensures working in small, testable chunks.

### Progress Notifications

When implementing long-running operations (>1 second):
- Use the ProgressSender from ToolContext to send progress notifications
- Send start notification (0% progress) when beginning operation
- Send periodic progress updates (with percentage when deterministic)
- Send completion notification (100% progress) when finished
- Do not fail the operation if notification sending fails
```

## Dependencies

Must be completed **after**:
- Phase 1: Implement MCP Progress Notification Infrastructure (01K7SHZ4203SMD2C6HTW1QV3ZP)

## Verification

- [ ] notify_create instructions removed
- [ ] Progress notification guidance added
- [ ] Memo is up to date with current practices

## Proposed Solution

The fix is straightforward - this is a simple text replacement in the CODING_STANDARDS memo:

1. Read the current memo content to confirm the exact text
2. Remove the line: `- notify_create the user that an issue was created using the notify_create tool`
3. Add a new section after the Refactoring section with guidance on Progress Notifications
4. The new section should explain when and how to use ProgressSender from ToolContext

This is a documentation-only change with no code implementation needed. The change aligns the standards with the removal of the notify_create tool and introduces guidance on using the new MCP progress notification infrastructure.
## Implementation Notes

Successfully completed the documentation update:

1. **Removed notify_create reference**: Deleted the line `- notify_create the user that an issue was created using the notify_create tool` from line 50 of the CODING_STANDARDS.md memo

2. **Added Progress Notifications section**: Added a new section after the Refactoring section with guidance on using ProgressSender from ToolContext for long-running operations

3. **Verification**: Confirmed the memo_get tool returns the updated content with:
   - The notify_create instruction removed
   - New Progress Notifications section in place with clear guidelines on when and how to use MCP progress notifications

The change aligns the coding standards with the removal of the notify_create tool and provides clear guidance for developers on implementing MCP progress notifications in long-running operations.

**Files Modified**:
- `.swissarmyhammer/memos/CODING_STANDARDS.md` - Updated refactoring section and added progress notifications guidance