# Research Current Tool Implementations

Refer to ./specification/issue_current.md

## Goal

Understand the complete implementation of `issue_current` and `issue_next` tools before consolidating them into `issue_show`.

## Tasks

1. **Analyze issue_current tool implementation**:
   - Read `/swissarmyhammer/src/mcp/tools/issues/current/mod.rs`
   - Understand git branch parsing logic
   - Identify current branch prefix handling logic
   - Document key functionality that needs to be preserved

2. **Analyze issue_next tool implementation**:
   - Read `/swissarmyhammer/src/mcp/tools/issues/next/mod.rs`
   - Understand next issue selection logic
   - Identify how it uses storage backend to find pending issues
   - Document key functionality that needs to be preserved

3. **Understand current issue_show tool**:
   - Read `/swissarmyhammer/src/mcp/tools/issues/show/mod.rs`
   - Understand current parameter handling
   - Identify extension points for new functionality
   - Document current behavior to ensure backward compatibility

4. **Document findings**:
   - Create detailed analysis of what needs to be integrated
   - Identify potential conflicts or edge cases
   - Plan the integration approach

## Expected Outcome

Clear understanding of:
- How current branch parsing works in `issue_current`
- How next issue selection works in `issue_next`
- Current `issue_show` implementation structure
- Integration strategy for consolidating functionality

## Success Criteria

- All three tool implementations are fully understood
- Integration approach is documented and planned
- No functionality will be lost in the consolidation
- Backward compatibility is preserved for existing `issue_show` usage