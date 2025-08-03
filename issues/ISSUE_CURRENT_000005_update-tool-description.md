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