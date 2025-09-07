# Remove memo_delete tool - memos should not be deletable

## Problem

The `memo_delete` tool exists and allows deletion of memos. This goes against the design principle that memos should be permanent records that cannot be deleted, only updated or superseded.

## Rationale

Memos are intended to be:
- Permanent knowledge artifacts
- Historical records of information and decisions
- Reference materials that should persist over time
- Immutable once created (except for content updates)

Allowing deletion undermines the integrity and reliability of the memo system as a knowledge base.

## Solution

Remove the `memo_delete` tool entirely:

1. Remove `memo_delete` from the MCP tools registry
2. Remove the implementation files
3. Update documentation to clarify memos are permanent
4. Consider adding a memo archiving or status system if needed instead

## Files to Remove/Update

- `swissarmyhammer-tools/src/mcp/tools/memoranda/delete/mod.rs`
- MCP tool registry entries for memo_delete
- Related tests for memo deletion
- Tool descriptions and documentation

## Alternative Approaches

If there's a legitimate need to "remove" memos:
- Add a memo status/archive system instead of deletion
- Allow memo content to be replaced with a tombstone message
- Implement memo versioning with deprecation

## Acceptance Criteria

- [ ] `memo_delete` tool completely removed from codebase
- [ ] Tool registry no longer includes memo_delete
- [ ] Tests for memo deletion removed
- [ ] Documentation updated to reflect permanent nature of memos
- [ ] No breaking changes to existing memo functionality
- [ ] Consider implementing memo archiving as alternative if needed