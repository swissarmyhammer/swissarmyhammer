# Step 4: Update .gitignore

Refer to ideas/eliminate-issues-and-memos-migration.md

## Goal

Add `.swissarmyhammer/issues/` and `.swissarmyhammer/memos/` directories to `.gitignore` so they are no longer tracked by git.

## Context

Following the "NO MIGRATION" approach, we don't delete existing issues and memos directories. Instead, we simply ignore them going forward. This allows:
- Zero risk of data loss
- Easy rollback if needed
- Existing directories remain for reference
- New systems (rules + todos) take over

## Changes Required

Add these lines to `.gitignore`:

```gitignore
# Issues and Memos (deprecated - replaced by Rules + Todos)
.swissarmyhammer/issues/
.swissarmyhammer/memos/
```

## Important Notes

According to the spec:
> **IMPORTANT** .swissarmyhammer/issues and .swissarmyhammer/memos are important metadata and should always be committed to git with your code changes

However, the migration plan states:
> Add `.swissarmyhammer/issues/` and `.swissarmyhammer/memos/` (stop tracking them)

This is intentional - we're deprecating these systems, so NEW projects should ignore these directories. Existing projects with historical data will keep it in git history.

## Implementation Tasks

1. **Update `.gitignore`**
   - Add entry for `.swissarmyhammer/issues/`
   - Add entry for `.swissarmyhammer/memos/`
   - Add comment explaining deprecation

2. **Verify changes**
   - Existing files remain in working directory
   - Git stops tracking new changes to these directories
   - Existing git history preserved

## Testing Checklist

- ✅ `.gitignore` updated with new entries
- ✅ `git status` doesn't show changes in issues/ or memos/
- ✅ Directories still exist on filesystem
- ✅ Can still read old files if needed
- ✅ New files in these directories are ignored

## Verification Commands

```bash
# Check .gitignore contains new entries
grep ".swissarmyhammer/issues/" .gitignore
grep ".swissarmyhammer/memos/" .gitignore

# Create test file and verify it's ignored
touch .swissarmyhammer/issues/test-ignored.md
git status | grep -v "test-ignored"

# Clean up test
rm .swissarmyhammer/issues/test-ignored.md
```

## Acceptance Criteria

- `.gitignore` contains `.swissarmyhammer/issues/`
- `.gitignore` contains `.swissarmyhammer/memos/`
- Git ignores new files in these directories
- Existing directories remain on filesystem
- All tests passing
- Build succeeds

## Estimated Changes

~5 lines (simple .gitignore addition)
