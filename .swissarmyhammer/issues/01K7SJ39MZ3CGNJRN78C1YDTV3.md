# Remove notify_create from CODING_STANDARDS Memo

## Parent Issue
Eliminate notify_create Tool and Replace with Native MCP Notifications (01K7SHXK4XKMDREMXG7BFJ8YJ7)

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
