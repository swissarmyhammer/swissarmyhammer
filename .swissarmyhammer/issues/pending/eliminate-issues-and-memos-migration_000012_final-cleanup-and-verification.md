# Step 12: Final Cleanup and Verification

Refer to ideas/eliminate-issues-and-memos-migration.md

## Goal

Perform final cleanup, fix any remaining broken tests, update documentation, and verify the entire migration is complete and functional.

## Context

This is the final step that ensures everything works correctly after removing the issues and memos systems. We need to verify the new architecture (rules + todos) is fully operational and all references to the old systems are gone.

## Implementation Tasks

### 1. Run Full Test Suite

```bash
# Run all tests across the workspace
cargo nextest run --workspace

# Check for any failures
# Fix any broken tests that reference issues or memos
```

### 2. Search for Remaining References

```bash
# Check for any issue/memo references in code
rg "issue_create|issue_show|issue_list|issue_update|issue_mark_complete|issue_all_complete" --type rust --type md
rg "memo_create|memo_get|memo_list|memo_get_all_context" --type rust --type md

# Check for references in comments
rg "issues system|memos system" --type rust --type md

# Check workflow and prompt references
rg "do_issue|implement|are_issues_complete" builtin/
```

### 3. Update Documentation

Check and update these documentation files:

**README.md** - Update architecture overview to mention rules + todos (not issues + memos)

**Architecture docs** (if any) - Update to reflect new systems

**Contributing guide** (if any) - Update workflow instructions

### 4. Verify New Workflows Work

Test the new architecture end-to-end:

```bash
# Test rule_create tool works
# Create test spec file
echo "# Test Spec\n\nTest requirement" > /tmp/test-spec.md

# Run plan workflow (should create rules + todos)
sah plan --plan-filename /tmp/test-spec.md

# Verify rules created
ls .swissarmyhammer/rules/

# Verify todos created
sah todo show --item next

# Test do workflow
sah do

# Clean up test data
```

### 5. Check for Lingering Files

```bash
# Check for any orphaned files related to issues/memos
find . -name "*issue*" -o -name "*memo*" | grep -v ".git" | grep -v "node_modules"

# Specifically check for:
# - Leftover test fixtures
# - Orphaned documentation
# - Old example files
```

### 6. Verify .gitignore Working

```bash
# Create test files in ignored directories
touch .swissarmyhammer/issues/test-ignored.md
touch .swissarmyhammer/memos/test-ignored.md

# Verify git ignores them
git status | grep -v "test-ignored"

# Clean up
rm .swissarmyhammer/issues/test-ignored.md
rm .swissarmyhammer/memos/test-ignored.md
```

### 7. Update CHANGELOG or Migration Notes

Create a migration note documenting the changes:

**`.swissarmyhammer/MIGRATION-NOTES.md`** (or similar):
```markdown
# Migration: Issues and Memos Removed

Date: [Current Date]

## Summary

The issues and memos systems have been removed and replaced with:
- **Rules**: Permanent executable specifications
- **Todos**: Ephemeral task tracking with rich markdown context

## What Changed

- Removed `swissarmyhammer-issues` crate
- Removed `swissarmyhammer-memoranda` crate
- Removed issue MCP tools
- Removed memo MCP tools
- Removed workflows: `do_issue`, `implement`
- Updated `plan` workflow to create rules + todos
- Renamed `do_todos` workflow to `do`

## Migration Path

No migration needed. Old `.swissarmyhammer/issues/` and `.swissarmyhammer/memos/` 
directories are now ignored but remain for reference.

## New Workflow

1. `sah plan spec.md` - Creates rules + todos
2. `sah do` - Works through todos
3. `sah review` - Checks rules
4. `sah test` - Runs tests
```

### 8. Run Build and Clippy

```bash
# Full workspace build
cargo build --workspace --release

# Run clippy for warnings
cargo clippy --workspace -- -D warnings

# Format code
cargo fmt --all

# Check formatting
cargo fmt --all -- --check
```

## Testing Checklist

- ✅ All workspace tests pass
- ✅ No references to old systems in code
- ✅ Documentation updated
- ✅ New workflows tested end-to-end
- ✅ No orphaned files found
- ✅ `.gitignore` working correctly
- ✅ Migration notes created
- ✅ Build succeeds with no warnings
- ✅ Clippy passes with no warnings
- ✅ Code properly formatted

## Verification Commands

```bash
# Full test suite
cargo nextest run --workspace

# Search for remaining references
rg "issue_create|memo_create" --type rust
rg "do_issue|implement" builtin/

# Verify new commands work
sah do --help
sah rule --help
sah todo --help

# Build and check
cargo build --workspace --release
cargo clippy --workspace
cargo fmt --all -- --check
```

## Acceptance Criteria

- All tests pass across entire workspace
- No references to issues/memos systems remain (except in .gitignore and migration notes)
- Documentation updated to reflect new architecture
- New workflows (plan, do, review) work end-to-end
- Build succeeds with no errors or warnings
- Clippy passes with no warnings
- Code properly formatted
- Migration notes documented

## Success Metrics

Final verification that:
- ✅ `rule_create` tool works
- ✅ `plan` workflow creates rules + todos
- ✅ `sah do` command works
- ✅ All issue-related code removed
- ✅ All memo-related code removed
- ✅ Build and tests clean
- ✅ No regressions in existing functionality

## Estimated Changes

~50-100 lines (documentation updates, test fixes, migration notes)
