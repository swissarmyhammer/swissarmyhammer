# Fix memo_create Tool to Return Simple OK Response

## Problem
The `memo_create` tool is disobeying instructions and returning complex responses with made-up data instead of the simple "OK" response that was requested. The tool is currently returning memo details including non-existent fields like "ID" when it should just return a success confirmation.

## Current Behavior (Wrong)
The memo_create tool currently returns complex responses like:
```json
{
  "memo": {
    "id": "some-made-up-id", 
    "title": "...",
    "content": "...",
    "created": "..."
  },
  "message": "Memo created successfully"
}
```

## Required Behavior (Correct)
The tool should return a simple OK response:
```json
{
  "message": "OK"
}
```

## Evidence of the Problem
- Tool was explicitly instructed to return simple OK responses
- Currently returning complex memo objects with fabricated data
- Making up fields like "ID" that may not actually exist in the memo system
- Over-engineering the response when simplicity was requested

## Root Cause Analysis
- [ ] Check if this is in the MCP tool implementation
- [ ] Verify if this is in the domain crate (`swissarmyhammer-memoranda`) 
- [ ] Determine if this is a response formatting issue in swissarmyhammer-tools
- [ ] Check if this violates the original requirements/specifications

## Implementation Plan

### Phase 1: Locate the Problem
- [ ] Find the `memo_create` tool implementation in swissarmyhammer-tools
- [ ] Identify where the complex response is being generated
- [ ] Check if the issue is in the MCP tool wrapper vs domain crate
- [ ] Verify what the actual memo creation returns vs what's being fabricated

### Phase 2: Fix the Response Format
- [ ] Update memo_create tool to return simple `{"message": "OK"}` response
- [ ] Remove any complex memo object returns
- [ ] Remove any made-up fields like "ID" that don't actually exist
- [ ] Ensure response format matches the original requirements

### Phase 3: Verify Domain Crate Behavior
- [ ] Check if `swissarmyhammer-memoranda` domain crate returns appropriate data
- [ ] Ensure MCP tool is not fabricating data that doesn't exist
- [ ] Verify memo creation actually succeeds before returning OK
- [ ] Don't return internal implementation details

### Phase 4: Testing
- [ ] Test memo_create tool returns exactly `{"message": "OK"}`
- [ ] Verify memo creation still works (just doesn't return details)
- [ ] Test error cases still return appropriate error messages
- [ ] Ensure no regression in actual memo functionality

### Phase 5: Documentation/Requirements Check
- [ ] Review original requirements for memo_create tool
- [ ] Update tool description if needed to reflect simple OK response
- [ ] Ensure consistency with other tools that were requested to return OK

## Files to Investigate
- `swissarmyhammer-tools/src/mcp/tools/memoranda/create/mod.rs` - Likely location of memo_create tool
- `swissarmyhammer-memoranda/` - Domain crate to understand actual return values
- Any MCP response formatting utilities
- Tool description files for memo_create

## Success Criteria
- [ ] memo_create tool returns simple `{"message": "OK"}` response
- [ ] No fabricated or made-up data in responses
- [ ] Memo creation functionality still works correctly
- [ ] Tool follows the original instructions for simple responses
- [ ] No regression in memo creation capabilities

## Risk Mitigation
- Ensure memo creation actually succeeds before returning OK
- Test that error cases still return meaningful error messages
- Verify the simplification doesn't break downstream consumers
- Keep actual memo creation functionality intact

## Notes
This is about following explicit instructions for response format simplicity. The tool should do what it was told to do - return "OK" for successful memo creation, not complex objects with potentially fabricated data.

The principle is: **do what was requested, nothing more, nothing less.**