# Fix File Operation Tools to Return Simple Responses

## Problem
The file operation tools (edit, write, read) are disobeying instructions by returning overly verbose, technical responses instead of simple confirmations. These tools provide unnecessary technical details when simple "OK" responses were requested.

## Current Behavior (Wrong)

### files/edit/mod.rs - Lines 450-452:
```rust
"Successfully edited file: {} | {} replacements made | {} bytes written | Encoding: {} | Line endings: {} | Metadata preserved: {}"
```

### files/write/mod.rs - Line 194:
```rust
"Successfully wrote {} bytes to {}"
```

### files/read/mod.rs - Line 213:
```rust
"Successfully read file content"
```

## Required Behavior (Correct)

### File Edit Tool:
Should return simple: `{"message": "OK"}`

### File Write Tool: 
Should return simple: `{"message": "OK"}`

### File Read Tool:
Should just return the file content without success announcements

## Evidence of Disobedience
- Tools were instructed to provide simple responses
- Currently returning technical implementation details (bytes written, encoding, line endings, replacements made)
- Over-engineering responses with unnecessary metadata
- Violating the principle: "do what was asked, nothing more, nothing less"

## Implementation Plan

### Phase 1: Fix File Edit Tool
- [ ] Update `swissarmyhammer-tools/src/mcp/tools/files/edit/mod.rs`
- [ ] Change verbose response on line ~450 to simple `"OK"`
- [ ] Remove technical details: replacements made, bytes written, encoding, line endings, metadata preservation
- [ ] Update tests to expect simple "OK" response
- [ ] Verify file editing functionality still works

### Phase 2: Fix File Write Tool
- [ ] Update `swissarmyhammer-tools/src/mcp/tools/files/write/mod.rs`
- [ ] Change response on line ~194 from detailed bytes/file info to simple `"OK"`
- [ ] Remove technical details about file size and path
- [ ] Update tests to expect simple "OK" response  
- [ ] Verify file writing functionality still works

### Phase 3: Fix File Read Tool
- [ ] Update `swissarmyhammer-tools/src/mcp/tools/files/read/mod.rs`
- [ ] Remove success announcement on line ~213
- [ ] Return only the file content without "Successfully read file content" message
- [ ] Update tests if they expect success messages
- [ ] Verify file reading functionality still works

### Phase 4: Update Tests
- [ ] Update test assertions in `files/edit/mod.rs` around line 907-909
- [ ] Update test assertions in `files/write/mod.rs` around line 584  
- [ ] Remove expectations for verbose technical details
- [ ] Tests should verify functionality works, not response verbosity

### Phase 5: Update Documentation
- [ ] Update tool descriptions in `files/edit/description.md`
- [ ] Update tool descriptions in `files/write/description.md`
- [ ] Update tool descriptions in `files/read/description.md`
- [ ] Remove examples showing verbose responses
- [ ] Show simple "OK" responses in examples

## Files to Update

### Core Implementation Files
- `src/mcp/tools/files/edit/mod.rs` - Simplify edit response
- `src/mcp/tools/files/write/mod.rs` - Simplify write response  
- `src/mcp/tools/files/read/mod.rs` - Remove success announcement

### Test Files
- Update test assertions that expect verbose responses
- Verify functionality still works with simple responses

### Documentation Files  
- `src/mcp/tools/files/edit/description.md` - Update examples
- `src/mcp/tools/files/write/description.md` - Update examples
- `src/mcp/tools/files/read/description.md` - Update examples

## Success Criteria
- [ ] File edit tool returns simple `{"message": "OK"}`
- [ ] File write tool returns simple `{"message": "OK"}`
- [ ] File read tool returns only content without success messages
- [ ] No technical implementation details in responses
- [ ] File operations continue to work correctly
- [ ] Tests pass with simple response expectations
- [ ] Documentation reflects simple response format

## Risk Mitigation
- Ensure file operations still work correctly after response changes
- Test edge cases and error scenarios
- Verify error messages are still informative
- Keep actual functionality intact while simplifying responses

## Notes
This addresses the core disobedience of providing complex technical details when simple confirmations were requested. The principle is: **do the work correctly, but respond simply**.

File operations should work exactly the same - only the response format changes from verbose technical details to simple "OK" confirmations.