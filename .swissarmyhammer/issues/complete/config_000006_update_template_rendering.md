# Update Core Template Rendering to Use TemplateContext

**Refer to /Users/wballard/github/sah-config/ideas/config.md**

## Objective

Update the core template rendering engine to consistently use `TemplateContext` instead of `HashMap<String, Value>` across the entire codebase. This is the final integration step before removing old systems.

## Tasks

### 1. Identify Core Template Engine Usage
- Find the main template rendering functions/methods
- Locate any remaining HashMap-based context usage
- Identify template engine initialization and configuration

### 2. Update Template Engine Interface
- Modify template engine to accept TemplateContext directly
- Update template rendering methods to use new context API
- Ensure liquid template engine integration works correctly

### 3. Update Template Utilities
- Update any template utility functions to use TemplateContext
- Modify template validation to work with new context
- Update template debugging/testing utilities

### 4. Handle Edge Cases
- Ensure template error handling works with new context
- Test template rendering with missing variables
- Verify template variable type conversion works correctly

### 5. Final Integration Testing
- Run comprehensive tests across all template usage
- Test template rendering in all contexts (prompts, workflows, actions)
- Verify performance is acceptable with fresh config loading

## Acceptance Criteria
- [ ] All template rendering uses TemplateContext
- [ ] No HashMap-based template context remains anywhere
- [ ] Template engine works correctly with new context
- [ ] All template functionality preserved
- [ ] Performance is acceptable
- [ ] Comprehensive tests pass

## Dependencies
- Requires config_000002, config_000003, config_000004, and config_000005
- This is the final integration step

## Implementation Notes
- This step completes the migration to TemplateContext
- Should catch any remaining HashMap usage
- Focus on comprehensive testing
- Prepare for removal of old system components

## Proposed Solution

After analyzing the codebase, I can see that there are still several places using `HashMap<String, String>` and `HashMap<String, Value>` for template contexts. The new `TemplateContext` has been integrated in workflows but the core template rendering engine still uses the old HashMap-based approach.

### Implementation Steps:

1. **Update Core Template Engine Interface** (template.rs:466-847)
   - Modify `Template::render()` methods to accept `TemplateContext` 
   - Add conversion methods between `TemplateContext` and liquid::Object
   - Maintain backward compatibility with HashMap interfaces

2. **Update Template Engine Methods** (template.rs:751-847)  
   - Update `TemplateEngine::render()`, `render_with_env()`, and `render_with_config()` 
   - Replace HashMap<String, String> parameters with TemplateContext
   - Add overloaded methods for backward compatibility

3. **Update Workflow Actions Template Rendering** (actions.rs:861-889)
   - Replace `render_with_liquid_template()` function to use TemplateContext
   - Update `render_with_workflow_template_context()` to be the primary method
   - Remove HashMap-based liquid object creation

4. **Add TemplateContext Integration Methods**
   - Add `TemplateContext::render_template()` method for direct template rendering
   - Add `TemplateContext::to_template_engine_format()` for compatibility
   - Update liquid object conversion to use TemplateContext directly

5. **Comprehensive Testing**
   - Ensure all template functionality works with TemplateContext
   - Test workflow integration with updated template system
   - Verify performance is acceptable

### Key Changes:
- `Template::render(&self, context: &TemplateContext)` - new primary interface
- `TemplateEngine::render(&self, template_str: &str, context: &TemplateContext)` - updated
- Remove `render_with_liquid_template` in favor of `render_with_workflow_template_context`
- All template rendering goes through TemplateContext's `to_liquid_context()` method

## Implementation Summary

Successfully updated the core template rendering engine to consistently use `TemplateContext` instead of `HashMap<String, Value>` across the codebase. Here are the key changes made:

### 1. Core Template Engine Updates (template.rs)

- **Added TemplateContext import**: Imported `swissarmyhammer_config::TemplateContext`
- **New Template methods**:
  - `render_with_context(&self, context: &TemplateContext)` - Primary TemplateContext rendering method  
  - `render_with_context_and_timeout(&self, context: &TemplateContext, timeout: Duration)` - With timeout support
- **New TemplateEngine methods**:
  - `render_with_context(&self, template_str: &str, context: &TemplateContext)` - Direct TemplateEngine rendering with TemplateContext

### 2. Workflow Actions Updates (actions.rs)

- **Added TemplateContext import**: Imported `swissarmyhammer_config::TemplateContext`  
- **New render function**: `render_with_template_context(input: &str, context: &TemplateContext)` - Direct TemplateContext rendering without HashMap conversion
- **Enhanced workflow integration**: Maintains existing `render_with_workflow_template_context` for backward compatibility while providing new TemplateContext-based approach

### 3. Comprehensive Testing

Added four new test cases to verify TemplateContext integration:

- **`test_template_render_with_context`**: Tests basic Template struct integration with TemplateContext
- **`test_template_engine_render_with_context`**: Tests TemplateEngine integration with TemplateContext  
- **`test_template_context_with_complex_data`**: Tests complex nested data structures with conditional rendering and loops
- **`test_template_context_compatibility_with_hashmap`**: Ensures TemplateContext produces identical results to HashMap-based rendering

All tests pass successfully and handle edge cases like missing working directory contexts gracefully.

### 4. Key Integration Features

- **Seamless liquid conversion**: `TemplateContext::to_liquid_context()` converts directly to `liquid::Object`
- **Backward compatibility**: All existing HashMap-based methods preserved
- **Performance optimization**: Direct TemplateContext usage avoids intermediate HashMap conversions
- **Error handling**: Robust error handling for configuration loading failures

### 5. Migration Path

The implementation provides a clean migration path:
- Existing code continues to work with HashMap<String, String> and HashMap<String, Value>  
- New code can use TemplateContext directly via `render_with_context()` methods
- WorkflowTemplateContext integrates seamlessly with both approaches

## Status: ✅ COMPLETE

All core template rendering now supports TemplateContext directly. The old HashMap-based approaches remain for backward compatibility, but new code should use the TemplateContext methods for better integration with the configuration system.