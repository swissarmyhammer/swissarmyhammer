# Step 2: Rename do_todos Workflow to do

Refer to ideas/eliminate-issues-and-memos-migration.md

## Goal

Rename the `do_todos` workflow to `do` to make it the primary implementation loop workflow. This simplifies the command and aligns with the new architecture where todos are the main task tracking mechanism.

## Context

In the new architecture, todos become the central task tracking system. The `do` workflow will be the main way to work through tasks, replacing the previous `do_issue` and `implement` workflows.

## Changes Required

```mermaid
graph LR
    A[do_todos.md] -->|rename| B[do.md]
    B -->|automatic| C[sah do command]
```

## Implementation Tasks

1. **Rename workflow file**
   - Rename `builtin/workflows/do_todos.md` to `builtin/workflows/do.md`
   - No content changes needed - the workflow logic remains the same

2. **Verify CLI integration**
   - The dynamic CLI will automatically create `sah do` command from the renamed file
   - Test that `sah do` command works
   - Verify old `sah do_todos` command no longer exists

3. **Update internal references**
   - Search for any hardcoded references to "do_todos" workflow name
   - Update them to "do" (if any exist)
   - Check workflow documentation

## Testing Checklist

- ✅ File renamed successfully
- ✅ `sah do` command available
- ✅ `sah do_todos` command no longer exists
- ✅ Workflow executes correctly with new name
- ✅ No broken references in codebase

## Verification Commands

```bash
# Check that do.md exists
ls builtin/workflows/do.md

# Check that do_todos.md doesn't exist
ls builtin/workflows/do_todos.md 2>&1 | grep "No such file"

# Verify CLI command works
sah do --help

# Verify old command is gone
sah do_todos --help 2>&1 | grep "unrecognized"
```

## Acceptance Criteria

- `builtin/workflows/do.md` exists
- `builtin/workflows/do_todos.md` does not exist
- `sah do` command works correctly
- `sah do_todos` command returns "unrecognized subcommand"
- All tests passing
- Build succeeds

## Estimated Changes

~2 lines (file rename only, minimal changes)
