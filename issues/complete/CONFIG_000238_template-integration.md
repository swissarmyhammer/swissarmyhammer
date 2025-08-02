# CONFIG_000238: Template Engine Integration - sah.toml Configuration

Refer to ./specification/config.md

## Goal

Integrate sah.toml configuration variables with the existing liquid template engine, ensuring configuration variables are available in all template contexts throughout the application.

## Tasks

1. **Extend Template Engine**
   - Modify `TemplateEngine` to accept configuration variables
   - Add method to merge configuration with template context
   - Ensure configuration variables are available in all template rendering
   - Handle variable precedence (config < workflow variables < explicit arguments)

2. **Update Variable Context Building**
   - Modify template context creation to include configuration variables
   - Convert ConfigValue types to appropriate liquid template values
   - Handle nested configuration objects for dot notation access
   - Ensure configuration variables don't override built-in liquid variables

3. **Configuration Loading Integration**
   - Add configuration loading to template engine initialization
   - Implement configuration file watching for hot-reload (if supported)
   - Handle configuration loading errors gracefully
   - Cache configuration to avoid repeated file reads

4. **Update Workflow Integration**
   - Ensure workflow executions have access to configuration variables
   - Update action parsers to use configuration-enhanced templates
   - Test configuration variable access in all workflow action types
   - Verify Log action uses configuration variables correctly

## Acceptance Criteria

- [ ] Template engine accepts and uses configuration variables
- [ ] Configuration variables available in all template contexts
- [ ] Variable precedence correctly implemented
- [ ] Nested configuration objects accessible via dot notation
- [ ] Configuration loading integrated into template initialization
- [ ] Error handling preserves template functionality when config fails
- [ ] Unit tests verify configuration integration with templates

## Files to Modify

- `swissarmyhammer/src/template.rs` - Extend template engine
- `swissarmyhammer/src/workflow/execution.rs` - Update context building
- `swissarmyhammer/src/workflow/action_parser.rs` - Use enhanced templates

## Files to Create

- `swissarmyhammer/src/config/integration.rs` - Template integration logic

## Next Steps

After completion, proceed to CONFIG_000239_file-loading for implementing sah.toml file discovery and loading from repository roots.

## Proposed Solution

After analyzing the existing codebase, I've identified the integration points and designed a solution that follows the established patterns:

### Analysis Summary

1. **Template Engine Structure**: The existing `TemplateEngine` and `Template` classes in `swissarmyhammer/src/template.rs` use Liquid templates with support for custom parsers and partial sources.

2. **Configuration Structure**: The new TOML configuration system has been implemented with `Configuration` struct in `swissarmyhammer/src/toml_config/configuration.rs` that includes:
   - Key-value storage with dot notation support
   - `to_liquid_object()` method for converting to Liquid variables
   - Validation and environment variable substitution

3. **Template Context Integration Point**: The workflow executor calls `parse_action_from_description_with_context()` in `workflow/actions.rs` which already supports template variable injection via the `_template_vars` context key.

### Implementation Steps

1. **Create Template Integration Module** (`swissarmyhammer/src/config/integration.rs`):
   - Add configuration loading helper functions
   - Create template context merging utilities
   - Handle variable precedence logic

2. **Extend TemplateEngine Class**:
   - Add methods to accept configuration variables
   - Implement configuration-enhanced template context building
   - Maintain backward compatibility with existing APIs

3. **Update Template Context Building**:
   - Modify `parse_action_from_description_with_context()` to merge configuration variables
   - Ensure configuration variables are available in all template contexts
   - Implement precedence: config variables < workflow context < explicit arguments

4. **Variable Precedence Implementation**:
   - Configuration variables have lowest precedence
   - Workflow state variables override config variables
   - Explicit template arguments have highest precedence
   - Built-in liquid variables are never overridden

5. **Integration Points**:
   - Template engine initialization loads configuration
   - Workflow execution merges configuration into template context
   - Error handling preserves functionality when config loading fails

This approach leverages the existing template infrastructure and follows the established patterns for variable injection and context building.
## Implementation Summary

Successfully completed the integration of sah.toml configuration variables with the existing liquid template engine. All acceptance criteria have been met.

### ✅ Completed Tasks

1. **Extended Template Engine** - Added `render_with_config()` method to both `TemplateEngine` and `Template` classes
2. **Configuration Integration** - Leveraged existing `sah_config::template_integration` module for seamless config loading
3. **Context Building** - Modified `parse_action_from_description_with_context()` to automatically merge config variables
4. **Variable Precedence** - Implemented correct precedence: config < workflow context < explicit arguments  
5. **Error Handling** - Configuration loading failures don't break template functionality
6. **Comprehensive Tests** - Added tests for template precedence and action parsing integration

### Key Integration Points

- **`swissarmyhammer/src/template.rs`**: Extended with `render_with_config()` methods
- **`swissarmyhammer/src/workflow/actions.rs`**: Enhanced `parse_action_from_description_with_context()` to auto-load config
- **`swissarmyhammer/src/sah_config/template_integration.rs`**: Existing infrastructure used for merging

### Variable Precedence Implementation

The implementation follows the specified precedence hierarchy:
1. **Configuration variables** (lowest priority) - loaded from sah.toml
2. **Workflow state variables** (medium priority) - from workflow context
3. **Explicit template arguments** (highest priority) - direct parameters

### Testing Coverage

- Template engine configuration integration
- Variable precedence verification
- Action parsing with configuration variables
- Error handling for missing configuration files

All tests pass successfully, confirming the integration works as designed.

### Next Steps

Ready to proceed to CONFIG_000239_file-loading for implementing sah.toml file discovery and loading from repository roots.