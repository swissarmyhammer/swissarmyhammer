# Create Modern Test Command Handler  

Refer to /Users/wballard/github/swissarmyhammer/ideas/cli_prompt.md

## Overview

Create a modern test command handler that uses CliContext for both prompt library access and output formatting. Clean up the parameter handling and integrate with the global arguments pattern.

## Current State

- Complex `run_test_command()` with manual argument parsing
- Direct integration with TemplateContext
- Recreates PromptLibrary and PromptResolver instead of reusing them
- Long parameter collection function with mixed concerns

## Goals

- Clean test command handler using CliContext
- Get prompt library from CliContext (don't recreate)
- Preserve all existing test functionality  
- Better separation of parameter collection and prompt rendering
- Integration with global verbose/debug flags
- Cleaner error handling and user feedback

## Implementation Steps

### 1. Create Test Handler Module

**File**: `swissarmyhammer-cli/src/commands/prompt/test.rs`

```rust
use crate::context::CliContext;
use crate::commands::prompt::cli::TestCommand;
use swissarmyhammer::interactive_prompts::InteractivePrompts;
use swissarmyhammer_common::{Parameter, ParameterError};
use std::collections::HashMap;
use anyhow::Result;

/// Execute the test command with the provided configuration
pub async fn execute_test_command(
    test_cmd: TestCommand,
    cli_context: &CliContext,
) -> Result<()> {
    let prompt_name = test_cmd.prompt_name
        .ok_or_else(|| anyhow::anyhow!("Prompt name is required"))?;

    if cli_context.verbose {
        println!("Testing prompt: {}", prompt_name);
    }

    // Get prompt library from CliContext (don't recreate)
    let library = cli_context.get_prompt_library()?;
    
    // Get the specific prompt
    let prompt = library
        .get(&prompt_name)
        .map_err(|e| anyhow::anyhow!("Failed to get prompt '{}': {}", prompt_name, e))?;

    if cli_context.debug {
        println!("Prompt parameters: {:#?}", prompt.get_parameters());
    }

    // Collect parameters
    let parameters = collect_test_parameters(&test_cmd, prompt.get_parameters(), cli_context)?;
    
    // Render the prompt using CliContext
    let rendered = cli_context.render_prompt(&prompt_name, &parameters)?;

    // Output the result
    output_rendered_prompt(&rendered, &test_cmd, cli_context)?;
    
    Ok(())
}

/// Collect parameters for the test, combining CLI args with interactive prompts
fn collect_test_parameters(
    test_cmd: &TestCommand,
    prompt_parameters: &[Parameter],
    cli_context: &CliContext,
) -> Result<HashMap<String, serde_json::Value>> {
    // Parse CLI variables
    let mut cli_parameters = parse_cli_variables(&test_cmd.vars)?;
    
    if cli_context.verbose && !cli_parameters.is_empty() {
        println!("CLI parameters: {:#?}", cli_parameters);
    }

    // Use InteractivePrompts to collect missing parameters
    let interactive_prompts = InteractivePrompts::with_max_attempts(false, 3);
    let all_parameters = collect_missing_parameters(
        &interactive_prompts,
        prompt_parameters,
        &cli_parameters,
        cli_context.verbose,
    )?;

    if cli_context.verbose {
        println!("Final parameters: {:#?}", all_parameters);
    }

    Ok(all_parameters)
}

// ... rest of implementation stays the same but uses cli_context.verbose instead of debug parameter
```

**Key Changes**: 
1. Use `cli_context.get_prompt_library()` instead of creating new PromptLibrary/PromptResolver
2. Use `cli_context.render_prompt()` for rendering with proper context merging
3. Use `cli_context.verbose` for debug output instead of separate debug parameter

### 2. Update CliContext for Prompt Operations

**File**: `swissarmyhammer-cli/src/context.rs` 

```rust
impl CliContext {
    /// Get the prompt library (reuse existing, don't recreate)
    pub fn get_prompt_library(&self) -> Result<&PromptLibrary> {
        // Should return reference to cached library
    }
    
    /// Render a prompt with parameters, merging with template context
    pub fn render_prompt(
        &self, 
        prompt_name: &str, 
        parameters: &HashMap<String, serde_json::Value>
    ) -> Result<String> {
        // Merge parameters with self.template_context and render
    }
}
```

## Testing Requirements

### Unit Tests
- Test parameter parsing and validation
- Test interactive/non-interactive parameter collection  
- Test all parameter type conversions
- Test error handling for invalid inputs
- Test integration with CliContext methods

### Integration Tests
- Test full test command execution
- Test file output functionality
- Test parameter collection workflows

## Success Criteria

1. ✅ Clean test command handler using CliContext for everything
2. ✅ No recreation of expensive PromptLibrary/PromptResolver objects  
3. ✅ All existing test functionality preserved
4. ✅ Better separation of parameter collection and rendering
5. ✅ Integration with global verbose flags from CliContext
6. ✅ Comprehensive unit test coverage
7. ✅ Ready for integration with main command router

## Files Created

- `swissarmyhammer-cli/src/commands/prompt/test.rs` - Test command handler

## Files Modified

- `swissarmyhammer-cli/src/commands/prompt/mod.rs` - Export test module
- `swissarmyhammer-cli/src/context.rs` - Add prompt library access methods

## Risk Mitigation

- Preserve all existing test functionality
- Comprehensive tests for parameter collection
- Graceful error handling for all failure modes

---

**Estimated Effort**: Large (400-500 lines including tests)
**Dependencies**: cli_prompt_000002_create_prompt_cli_module, cli_prompt_000001_add_global_format_argument
**Blocks**: cli_prompt_000006_update_main_command_routing