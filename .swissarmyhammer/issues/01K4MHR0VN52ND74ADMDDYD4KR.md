# Fix Web Tool Responses to Return Simple Format

## Problem
The web fetch tools are disobeying instructions by returning overly verbose responses with unnecessary technical details instead of simple content delivery. These tools provide performance metrics and success announcements when they should just return the fetched content.

## Current Behavior (Wrong)

### web_fetch/fetch/mod.rs - Lines 158-159:
```rust
"Successfully fetched content from {} ({}ms, {} bytes, {} words, {:.1} KB/s)"
```

### web_fetch/fetch/mod.rs - Line 185:
```rust
"Successfully fetched content from URL"
```

### Description files show verbose examples:
```json
{
  "content": [{"type": "text", "text": "Successfully fetched content from URL"}],
  "is_error": false
}
```

## Required Behavior (Correct)

### Web Fetch Tool:
Should just return the fetched content without:
- Performance metrics (ms, bytes, words, KB/s)
- Success announcements ("Successfully fetched content from...")
- Technical implementation details

The response should contain only the actual fetched content that was requested.

## Evidence of Disobedience
- Tools were instructed to provide simple, clean responses
- Currently returning performance metrics and technical details
- Adding success announcements when user just wants the content
- Over-engineering responses with implementation details
- Violating the principle: "do what was asked, nothing more, nothing less"

## Implementation Plan

### Phase 1: Fix Web Fetch Response Format
- [ ] Update `swissarmyhammer-tools/src/mcp/tools/web_fetch/fetch/mod.rs`
- [ ] Remove verbose success message on line ~158-159
- [ ] Remove performance metrics (ms, bytes, words, KB/s)
- [ ] Remove "Successfully fetched content from URL" announcement on line ~185
- [ ] Return only the actual fetched content
- [ ] Keep error handling intact for failed fetches

### Phase 2: Clean Up Response Construction
- [ ] Review response building logic in web_fetch tool
- [ ] Ensure only the actual web content is returned
- [ ] Remove any metadata about the fetch operation
- [ ] Keep the content format clean and simple
- [ ] Maintain proper error responses for failures

### Phase 3: Update Tests
- [ ] Update test expectations in web fetch tests
- [ ] Remove assertions for verbose success messages
- [ ] Update test assertions in `tests/web_fetch_specification_compliance.rs` line ~140
- [ ] Tests should verify content is fetched, not response verbosity
- [ ] Ensure error case tests still work

### Phase 4: Update Documentation
- [ ] Update `src/mcp/tools/web_fetch/fetch/description.md`
- [ ] Remove examples showing verbose "Successfully fetched" responses
- [ ] Show clean content-only responses in examples
- [ ] Update response format documentation
- [ ] Ensure examples reflect simple content delivery

### Phase 5: Review Related Web Tools
- [ ] Check if web_search tools have similar verbosity issues
- [ ] Review any other web-related tools for consistent response format
- [ ] Ensure all web tools follow simple response principles

## Files to Update

### Core Implementation Files
- `src/mcp/tools/web_fetch/fetch/mod.rs` - Remove verbose success messages and metrics
- Check other web tool implementations for similar issues

### Test Files  
- `tests/web_fetch_specification_compliance.rs` - Update test expectations
- Remove assertions for verbose success announcements
- Focus tests on actual content delivery

### Documentation Files
- `src/mcp/tools/web_fetch/fetch/description.md` - Update examples
- Remove verbose response examples
- Show clean content-only responses

## Success Criteria
- [ ] Web fetch tool returns only the fetched content
- [ ] No performance metrics in responses (ms, bytes, words, KB/s)
- [ ] No "Successfully fetched" announcements
- [ ] No technical implementation details in responses
- [ ] Web fetching functionality continues to work correctly
- [ ] Error cases still return appropriate error messages
- [ ] Tests pass with simple response expectations
- [ ] Documentation reflects clean response format

## Risk Mitigation
- Ensure web fetching still works correctly after response changes
- Test various URL types and content formats
- Verify error handling remains informative
- Test edge cases like redirects, large content, slow responses
- Keep actual fetching functionality intact while simplifying responses

## Notes
This addresses the disobedience of adding unnecessary technical details and success announcements to web fetch responses. Users want the web content, not performance reports or success confirmations.

The principle is: **fetch the content correctly, but respond with just the content** - no need for technical metrics or success announcements.

Web tools should be transparent - users should get the content they requested without implementation details cluttering the response.