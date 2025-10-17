# Remove notify_create from Documentation - Tools Reference

## Parent Issue
Eliminate notify_create Tool and Replace with Native MCP Notifications (01K7SHXK4XKMDREMXG7BFJ8YJ7)

## Summary
Remove notify_create tool documentation from the tools reference page.

## Locations
- `swissarmyhammer-tools/doc/src/reference/tools.md` (around line 556)
- `doc/src/05-tools/notification-tools/create.md`
- `doc/src/05-tools/overview.md`

## Tasks

### 1. Remove from tools.md
File: `swissarmyhammer-tools/doc/src/reference/tools.md`

Remove the entire "Notifications" section (lines ~553-580):
```markdown
## Notifications

### notify_create

Send notification messages from LLM to user.

**Parameters**:
- `message` (string, required): The message to notify the user about
- `level` (string, optional): The notification level (info, warn, error)
- `context` (object, optional): Optional structured JSON data

**Returns**: Confirmation message

**Example**:
...
```

### 2. Delete notification tools documentation
Delete: `doc/src/05-tools/notification-tools/create.md`

### 3. Update tools overview
File: `doc/src/05-tools/overview.md`
- Remove notification-tools section
- Remove any references to notify_create

### 4. Update SUMMARY.md
File: `doc/src/SUMMARY.md`
- Remove link to notification tools documentation

## Dependencies

Must be completed **after**:
- Remove notify_create from Tool Registry

## Verification

- [ ] All notify_create documentation removed
- [ ] `mdbook build doc/` succeeds without broken links
- [ ] No dead links in documentation
- [ ] Table of contents updated
