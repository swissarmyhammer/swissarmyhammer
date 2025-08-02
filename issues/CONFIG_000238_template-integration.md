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