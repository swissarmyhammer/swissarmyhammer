# Migration: Issues and Memos Removed

Date: 2025-11-19
Status: ✅ Complete

## Summary

The issues and memos systems have been successfully removed and replaced with:
- **Rules**: Permanent executable specifications stored in `.swissarmyhammer/rules/`
- **Todos**: Ephemeral task tracking with rich markdown context stored in `.swissarmyhammer/todo.yaml`

## What Changed

### Removed Systems

**Issues System:**
- `swissarmyhammer-issues` crate (deleted)
- Issue MCP tools: `issue_create`, `issue_show`, `issue_list`, `issue_update`, `issue_mark_complete`
- Workflows: `do_issue`, `implement`
- Prompts: Various issue-related prompts

**Memos System:**
- `swissarmyhammer-memoranda` crate (deleted)
- Memo MCP tools: `memo_create`, `memo_get`, `memo_list`, `memo_get_all_context`
- Memo storage infrastructure

### New Systems

**Rules System:**
- Stored in `.swissarmyhammer/rules/` as markdown files
- Checked with `sah review` or `rules_check` MCP tool
- Define permanent acceptance criteria
- Support severity levels (error, warning, info, hint)
- Organize with tags and categories

**Todos System:**
- Stored in `.swissarmyhammer/todo.yaml`
- Managed with `todo_create`, `todo_show`, `todo_mark_complete` MCP tools
- Support rich markdown context
- Ephemeral - auto-deleted when complete
- Track implementation progress

### Updated Workflows

**`plan` Workflow:**
- Updated to create rules (not issues)
- Creates todos for implementation steps
- Executes `plan` prompt with new behavior

**`do` Workflow:**
- Renamed from `do_todos`
- Works through todos autonomously
- No longer depends on issues

**`review` Workflow:**
- Checks rules (not issues)
- Verifies acceptance criteria

## Migration Path

**For Users:**

No migration needed for existing users. The old `.swissarmyhammer/issues/` and `.swissarmyhammer/memos/` directories are now gitignored but remain for historical reference.

**New Workflow:**

1. Write a specification in markdown
2. Run `sah plan spec.md` - Creates rules + todos
3. Run `sah do` - Works through todos autonomously
4. Run `sah review` - Checks rules are satisfied
5. Run `sah test` - Runs tests

## Benefits

**Rules over Issues:**
- Permanent and executable
- Automatically checked against code
- Define "done" criteria clearly
- Support multiple severity levels

**Todos over Memos:**
- Ephemeral - no clutter
- Rich markdown context
- Better suited for task tracking
- Auto-cleanup when complete

## Files Affected

- README.md - Updated architecture description
- doc/src/01-getting-started/introduction.md - Removed memo references
- Cargo.toml - Removed old crates
- Various test files - Updated for new architecture

## Verification

All verification criteria passed:
- ✅ Tests pass
- ✅ Build succeeds with no warnings
- ✅ Clippy passes
- ✅ Code properly formatted
- ✅ No remaining references to old systems
- ✅ Documentation updated
- ✅ New architecture functional end-to-end

## References

- Original Migration Plan: `ideas/eliminate-issues-and-memos-migration.md`
- Specification: `.swissarmyhammer/issues/pending/eliminate-issues-and-memos-migration_000012_final-cleanup-and-verification.md`
- Rule: `.swissarmyhammer/rules/eliminate-issues-memos/final-verification-complete.md`
