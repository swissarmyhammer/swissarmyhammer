# LOG_000242: Fix Log Action Liquid Rendering

Refer to existing issue 01K1KQM85501ECE8XJGNZKNJQZ: Log actions need to render with liquid, using the current workflow variable context and all available variables.

## Goal

Fix the Log action implementation to properly render liquid templates using the current workflow variable context, ensuring that template variables like `{{branch_value}}` are rendered instead of being printed literally.

## Current Problem

Log actions are currently printing liquid template syntax literally instead of rendering it. For example:
- Current: `Branch 1 selected: {{branch_value}} contains Cargo`
- Expected: `Branch 1 selected: main contains Cargo`

## Tasks

1. **Analyze Current Log Action Implementation**
   - Examine `swissarmyhammer/src/workflow/actions.rs` LogAction implementation
   - Understand how other actions (like PromptAction) handle liquid rendering
   - Review the action parsing process in `action_parser.rs`
   - Identify where template rendering should occur

2. **Update LogAction Structure**
   - Modify LogAction to use template engine for message rendering
   - Ensure LogAction has access to current workflow variable context
   - Update LogAction execution to render templates before logging
   - Maintain backward compatibility for non-template log messages

3. **Integrate Template Engine**
   - Add template engine dependency to LogAction execution
   - Pass current workflow context to template rendering
   - Handle template rendering errors gracefully (fall back to literal text)
   - Ensure all available variables (workflow, configuration, built-in) are accessible

4. **Update Action Parser**
   - Ensure LogAction parsing preserves template syntax for later rendering
   - Don't pre-render templates during parsing phase
   - Pass template context during execution phase
   - Handle both quoted and unquoted log message formats

5. **Test Template Rendering**
   - Test with simple variable substitution (`{{variable}}`)
   - Test with liquid filters (`{{variable | default: 'fallback'}}`)
   - Test with complex expressions and conditionals
   - Test error handling for invalid template syntax

## Acceptance Criteria

- [ ] Log actions render liquid templates using current workflow context
- [ ] Template variables are properly substituted with actual values
- [ ] Invalid template syntax falls back to literal text gracefully
- [ ] All workflow variables, configuration variables, and built-ins accessible
- [ ] Existing non-template log messages continue to work unchanged
- [ ] Comprehensive test coverage for template rendering scenarios

## Files to Examine/Modify

- `swissarmyhammer/src/workflow/actions.rs` - LogAction implementation
- `swissarmyhammer/src/workflow/action_parser.rs` - Action parsing logic
- `swissarmyhammer/src/workflow/execution.rs` - Workflow execution context
- `swissarmyhammer/src/template.rs` - Template engine integration

## Test Cases to Address

- Basic variable substitution: `Log "Hello {{user_name}}"`
- Default values: `Log "Count: {{count | default: '0'}}"`
- Complex expressions: `Log "Status: {% if success %}OK{% else %}FAIL{% endif %}"`
- Invalid syntax: `Log "Broken: {{unclosed"`

## Next Steps

This addresses the immediate liquid rendering issue. After completion, the Log actions will properly render templates like other workflow actions.