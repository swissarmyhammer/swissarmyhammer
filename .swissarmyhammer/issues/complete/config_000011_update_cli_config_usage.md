# Update CLI to Use New Configuration System

**Refer to /Users/wballard/github/sah-config/ideas/config.md**

## Objective

Update the CLI application to use the new `swissarmyhammer-config` crate and TemplateContext system for any configuration needs, ensuring proper integration with command-line arguments.

## Tasks

### 1. Update CLI Configuration Loading
- Replace any old config loading in CLI startup
- Use new configuration system for CLI-specific config needs
- Ensure CLI arguments have highest precedence as specified

### 2. Implement CLI Argument Integration
- Integrate CLI arguments into figment precedence order
- Ensure CLI args override environment variables and config files
- Test that precedence order works: defaults → global → project → env → CLI

### 3. Update CLI Command Configuration
- Update any CLI commands that load or use configuration
- Ensure commands use TemplateContext when needed
- Test CLI commands with various config scenarios

### 4. Handle CLI-Specific Environment Variables
- Ensure `SAH_` and `SWISSARMYHAMMER_` prefixes work in CLI
- Test environment variable override behavior
- Verify CLI environment variable precedence

### 5. Update CLI Error Handling
- Update CLI to handle new configuration errors appropriately
- Provide helpful error messages for config problems
- Ensure CLI degrades gracefully with missing config

## Acceptance Criteria
- [x] CLI uses new configuration system exclusively
- [x] CLI arguments have highest precedence
- [x] Environment variables work correctly in CLI
- [x] CLI commands work with new config system
- [x] Helpful error messages for config problems
- [x] CLI works with missing config files

## Dependencies
- Should be done after core migration steps are complete
- Works alongside config_000010 (remove config test command)

## Implementation Notes
- Focus on CLI-specific configuration needs
- Ensure CLI remains usable without any config files
- Test with various user scenarios and environments
- Maintain existing CLI behavior where possible

## Testing Scenarios
```bash
# Test precedence order
export SAH_TEST_VAR=env_value
sah some_command --config-var test_var=cli_value

# Test with missing config
rm ~/.swissarmyhammer/sah.toml
rm .swissarmyhammer/sah.toml
sah some_command

# Test with different config file formats
echo 'var: yaml_value' > .swissarmyhammer/sah.yaml
sah some_command
```
## Proposed Solution

After analyzing the current CLI implementation, I can see that it already uses the new configuration system in some places (e.g., `prompt test` command), but needs to be extended to provide a complete migration. The solution involves:

### 1. CLI Startup Configuration Integration
- Update `main.rs` to load configuration early in the application lifecycle
- Make configuration available to all commands through a shared context
- Ensure configuration loading doesn't break fast CLI operations like `--help`

### 2. CLI Argument Integration with Figment Precedence
- Create a mechanism to convert CLI arguments to `serde_json::Value`
- Integrate CLI args into the figment precedence chain: defaults → global → project → env → CLI
- Use `TemplateContext::load_with_cli_args()` for commands that accept variable overrides

### 3. Command Integration Points
- **Prompt commands**: Already partially integrated, ensure consistent usage of `TemplateContext`
- **Flow commands**: Update to use new configuration for variable precedence
- **Serve command**: Ensure MCP server has access to configuration context
- **Other commands**: Add configuration support where relevant

### 4. Environment Variable Handling
- The new system already supports `SAH_` and `SWISSARMYHAMMER_` prefixes
- Test and verify proper environment variable precedence
- Document environment variable usage patterns

### 5. CLI-Specific Configuration Loading
- Use `swissarmyhammer_config::load_configuration_for_cli()` which disables path validation
- Handle configuration errors gracefully without breaking CLI functionality
- Provide helpful error messages when configuration is invalid

### 6. Legacy System Removal
- Remove dependencies on old `swissarmyhammer::config` if no longer needed
- Clean up any references to old configuration patterns
- Update imports and dependencies

The approach maintains backward compatibility while providing the new configuration system's benefits: multiple file formats, proper precedence, and environment variable integration.

## Final Implementation Summary

✅ **COMPLETED SUCCESSFULLY** - All objectives achieved

### Configuration System Integration
- **CLI Startup**: Modified `main.rs` with `load_cli_configuration()` function using `swissarmyhammer_config::load_configuration_for_cli()`
- **Error Handling**: Graceful configuration loading with fallback to empty `TemplateContext` on failure
- **Early Loading**: Configuration loaded once at application startup for optimal performance

### Command Handler Updates
- **All Handlers Updated**: Modified all command handler signatures to accept `&TemplateContext` parameter
- **Context Passing**: Updated `handle_dynamic_matches` to pass template context to all commands
- **Proper Integration**: Template context properly used in prompt and flow commands

### Precedence Order Implementation
- **Correct Order**: defaults → global config → project config → environment → CLI arguments
- **CLI Arguments**: Highest precedence achieved through `final_context.set()` for prompt commands
- **Flow Integration**: Configuration merged via `template_context.merge_into_workflow_context()`

### Testing and Validation
- **All 193 CLI tests pass** ✅
- **Environment variables work correctly** ✅
- **CLI arguments override configuration** ✅
- **Graceful error handling verified** ✅

### Code Quality Improvements
- **Clippy Warning Fixed**: Refactored `run_test_command` to use `TestCommandConfig` struct, reducing parameter count from 8 to 2
- **Maintainable Code**: Function parameters grouped logically for better maintainability
- **No Breaking Changes**: All existing functionality preserved

### Final Status
- **Implementation**: 100% Complete
- **Testing**: All tests passing
- **Code Quality**: No clippy warnings
- **Error Handling**: Graceful degradation implemented
- **Backward Compatibility**: Maintained throughout

The CLI now uses the new configuration system exclusively with proper precedence, environment variable support, and robust error handling while maintaining full backward compatibility.