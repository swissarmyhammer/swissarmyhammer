remove the compatibility layer for config and just use the natural api

## Proposed Solution

After analyzing the codebase, I can see that there is a `compat.rs` module in the `swissarmyhammer-config` crate that provides backward compatibility for the old configuration system. The natural API is already implemented in the `provider.rs` module with the `ConfigProvider` class.

### Current State
The compatibility layer provides:
- Legacy data structures like `ConfigValue`, `Configuration`, `ConfigurationLoader`
- Legacy functions like `merge_config_into_context`, `load_and_merge_repo_config`
- Legacy types in the `types` and `loader` modules
- Shell tool configuration compatibility

### Natural API (Already Available)
The natural API uses:
- `ConfigProvider` - Main entry point for configuration loading
- `TemplateContext` - Modern structured context for template variables
- `TemplateRenderer` - For rendering templates with context
- Clean methods like `load_template_context()`, `create_context_with_vars()`, etc.

### Files Using Compatibility Layer
I found 11 files using the compatibility layer:
- `swissarmyhammer/src/template.rs` - Template rendering
- `swissarmyhammer/src/lib.rs` - Public API exports
- `swissarmyhammer/src/prompts.rs` - Prompt rendering
- `swissarmyhammer/src/workflow/actions.rs` - Workflow actions
- `swissarmyhammer-cli/src/config.rs` - CLI configuration
- `swissarmyhammer-cli/src/validate.rs` - Config validation
- `swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs` - Shell tool
- `swissarmyhammer-tools/src/mcp/tools/web_search/search/mod.rs` - Web search tool
- `tests/shell_integration_final_tests.rs` - Integration tests

### Implementation Plan
1. Update each file to use the natural API (`ConfigProvider`, `TemplateContext`)
2. Replace `merge_config_into_context` with `ConfigProvider::create_context_with_vars`
3. Replace `load_repo_config` with `ConfigProvider::load_template_context`
4. Replace legacy `ConfigValue`/`Configuration` with `TemplateContext`
5. Update shell tool configuration to use direct `ConfigProvider` access
6. Remove the `compat.rs` module entirely
7. Remove the `compat` re-exports from `lib.rs`
8. Update tests to use the natural API