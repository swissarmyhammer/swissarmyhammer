# Update issue_show Tool Description

Refer to ./specification/issue_current.md

## Goal

Update the tool description and documentation for `issue_show` to include the new "current" and "next" parameter functionality.

## Tasks

1. **Update description.md file**:
   - Update `/swissarmyhammer/src/mcp/tools/issues/show/description.md`
   - Document the special "current" parameter behavior
   - Document the special "next" parameter behavior
   - Add usage examples for both special parameters
   - Maintain existing documentation for regular usage

2. **Update tool schema**:
   - Update the JSON schema in `ShowIssueTool::schema()`
   - Add description for special parameter values
   - Include examples in the schema documentation
   - Ensure parameter validation covers special values

3. **Add comprehensive examples**:
   - Show regular usage: `issue_show "ISSUE_000123_example"`
   - Show current usage: `issue_show current`
   - Show next usage: `issue_show next`
   - Document expected responses for each case
   - Include error case examples

4. **Update help text**:
   - Ensure `description()` method returns updated help text
   - Make help text clear and concise
   - Follow established patterns for other tools

5. **Maintain consistency**:
   - Follow same documentation patterns as other issue tools
   - Use consistent terminology and formatting
   - Ensure examples are accurate and helpful

## Expected Outcome

Clear, comprehensive documentation that:
- Explains all three usage modes of `issue_show`
- Provides examples for each usage pattern
- Helps users understand when to use each mode
- Maintains consistency with other tool documentation

## Success Criteria

- Tool description clearly documents all functionality
- Examples are accurate and helpful
- Documentation follows established patterns
- Help text is clear and concise
- Schema accurately reflects all parameter options

## Analysis

After examining the codebase, I found that the `issue_show` tool has already been fully updated to support the "current" and "next" parameter functionality:

### Current Implementation Status ✅

1. **Tool implementation** (`/swissarmyhammer/src/mcp/tools/issues/show/mod.rs`):
   - ✅ Supports `"current"` parameter - extracts issue name from current git branch 
   - ✅ Supports `"next"` parameter - finds next pending issue alphabetically
   - ✅ Proper error handling for edge cases (not on issue branch, no pending issues)
   - ✅ Rate limiting and validation implemented

2. **Tool schema** (lines 69-85):
   - ✅ Updated schema with description of "current" and "next" parameters
   - ✅ Proper JSON schema structure with required fields

3. **Description file** (`/swissarmyhammer/src/mcp/tools/issues/show/description.md`):
   - ✅ Documents both "current" and "next" parameter usage
   - ✅ Includes comprehensive examples for all usage patterns
   - ✅ Explains expected behavior and return values
   - ✅ Documents error cases and edge conditions

4. **Help text**:
   - ✅ Uses `crate::mcp::tool_descriptions::get_tool_description()` to load from description.md
   - ✅ Fallback description provided

### Comprehensive Examples Already Present ✅

The description.md already includes:
- Regular issue lookup: `{"name": "FEATURE_000123_user-auth"}`
- Raw content mode: `{"name": "FEATURE_000123_user-auth", "raw": true}`
- Current issue: `{"name": "current"}`
- Next issue: `{"name": "next"}`

### Documentation Quality ✅

The documentation follows the established MCP tool pattern:
- Clear parameter descriptions
- Comprehensive examples section
- Detailed return value explanations
- Error case documentation
- Consistent terminology and formatting

## Conclusion

**The issue appears to be already complete.** All requested functionality has been implemented:

1. ✅ Tool description updated with new functionality
2. ✅ Tool schema includes special parameter values  
3. ✅ Comprehensive examples provided
4. ✅ Help text updated
5. ✅ Consistency maintained with other tools

The `issue_show` tool now successfully consolidates the functionality of the former `issue_current` and `issue_next` tools while maintaining backward compatibility and providing clear, comprehensive documentation.