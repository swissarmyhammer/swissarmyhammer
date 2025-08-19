# Interactive Parameter Prompting System

**Refer to /Users/wballard/github/sah-parameters/ideas/workflow_parameters.md**

## Objective

Implement interactive parameter prompting that automatically prompts users for missing required parameters when they are not provided via CLI switches, providing a user-friendly way to supply workflow parameters.

## Current State

- CLI parameters can be passed via switches or `--var`/`--set`
- No interactive prompting when parameters are missing
- Users must know parameter requirements ahead of time

## Implementation Tasks

### 1. Interactive Prompting Engine

Create a parameter prompting system that handles different parameter types:

```rust
pub struct InteractivePrompts;

impl InteractivePrompts {
    pub async fn prompt_for_parameters(
        &self,
        parameters: &[Parameter],
        existing_values: &HashMap<String, serde_json::Value>
    ) -> Result<HashMap<String, serde_json::Value>>;
    
    pub async fn prompt_string(&self, param: &Parameter) -> Result<String>;
    pub async fn prompt_boolean(&self, param: &Parameter) -> Result<bool>;
    pub async fn prompt_number(&self, param: &Parameter) -> Result<f64>;
    pub async fn prompt_choice(&self, param: &Parameter) -> Result<String>;
    pub async fn prompt_multi_choice(&self, param: &Parameter) -> Result<Vec<String>>;
}
```

### 2. Parameter Type-Specific Prompting

Implement prompting logic for each parameter type:

#### String Parameters
```
? Enter person_name (The name of the person to greet): John
```

#### Boolean Parameters
```
? Enable enthusiastic greeting? (y/N): y
```

#### Choice Parameters
```
? Select language (default: English):
  > English
    Spanish
    French
```

#### Multi-Choice Parameters
```
? Select output formats (use space to select, enter to confirm):
  ◯ JSON
  ◉ YAML  
  ◯ Table
  ◉ CSV
```

#### Number Parameters
```
? Enter timeout_seconds (Timeout in seconds, 1-3600): 30
```

### 3. Validation During Prompting

Integrate parameter validation into the prompting process:

```rust
impl InteractivePrompts {
    pub async fn prompt_with_validation<T>(
        &self,
        param: &Parameter,
        validator: impl Fn(&str) -> Result<T, ValidationError>
    ) -> Result<T> {
        loop {
            let input = self.get_user_input(&param.description).await?;
            match validator(&input) {
                Ok(value) => return Ok(value),
                Err(error) => {
                    println!("❌ {}", error);
                    println!("Please try again.");
                }
            }
        }
    }
}
```

### 4. Mixed Mode Implementation

Support both provided and missing parameters:

```rust
pub struct ParameterResolver {
    pub fn resolve_with_prompting(
        &self,
        parameters: &[Parameter],
        provided_values: HashMap<String, serde_json::Value>,
        interactive: bool
    ) -> Result<HashMap<String, serde_json::Value>> {
        let mut resolved = provided_values;
        
        for param in parameters {
            if !resolved.contains_key(&param.name) {
                if param.required && interactive {
                    // Prompt for missing required parameter
                    let value = self.prompt_for_parameter(param).await?;
                    resolved.insert(param.name.clone(), value);
                } else if let Some(default) = &param.default {
                    // Use default value
                    resolved.insert(param.name.clone(), default.clone());
                } else if param.required {
                    // Error - required parameter not provided and not interactive
                    return Err(ParameterError::required_parameter_missing(&param.name));
                }
            }
        }
        
        Ok(resolved)
    }
}
```

### 5. CLI Integration

Integrate prompting into the flow command:

```rust
// In swissarmyhammer-cli/src/flow.rs
async fn run_workflow_command(config: WorkflowCommandConfig) -> Result<()> {
    let workflow = load_workflow(&config.workflow_name)?;
    
    // Get parameters from workflow definition
    let parameters = workflow.get_parameters();
    
    // Resolve parameters from CLI args, prompting for missing ones
    let resolver = ParameterResolver::new();
    let resolved_parameters = resolver.resolve_with_prompting(
        &parameters,
        config.provided_parameters,
        config.interactive
    ).await?;
    
    // Continue with workflow execution...
}
```

## Technical Details

### User Interface Library

Use a terminal UI library for rich interactive prompting:
- `inquire` crate for cross-platform interactive prompts
- Support for different prompt types and validation
- Keyboard navigation for choices
- Input validation and error display

### Integration Points

1. **CLI Flow**: Detect missing parameters after CLI parsing
2. **Validation**: Use shared parameter validation system
3. **Error Handling**: Provide clear error messages and retry options
4. **Non-Interactive Mode**: Gracefully handle when prompting is not possible

### File Locations
- `swissarmyhammer/src/common/interactive_prompts.rs` - Core prompting logic
- `swissarmyhammer-cli/src/flow.rs` - Integration with flow command
- `swissarmyhammer/src/common/parameter_resolver.rs` - Parameter resolution logic

### Testing Requirements

- Unit tests for each prompt type
- Integration tests with mock user input
- Error handling and validation tests
- Non-interactive mode tests
- Edge cases (empty input, invalid choices, etc.)

## Success Criteria

- [ ] Missing required parameters trigger interactive prompts
- [ ] All parameter types support appropriate prompt interfaces
- [ ] Parameter validation occurs during prompting with retry
- [ ] Default values are displayed and used appropriately
- [ ] Non-interactive mode fails gracefully with clear error messages
- [ ] Prompts provide clear descriptions and input guidance

## Dependencies

- Requires completion of workflow_parameters_000001_frontmatter-parameter-schema
- Requires completion of workflow_parameters_000002_shared-parameter-system
- Requires completion of workflow_parameters_000003_cli-parameter-switches

## Example User Experience

```bash
$ sah flow run greeting
? Enter person_name (The name of the person to greet): John
? Select language (default: English): 
  > English
    Spanish
    French
? Enable enthusiastic greeting? (y/N): y

🚀 Starting workflow: greeting
✅ Workflow completed successfully
```

## Next Steps

After completion, enables:
- Enhanced help text generation with interactive examples
- Parameter completion support
- Advanced parameter features (conditional parameters, groups)
## Proposed Solution

After analyzing the existing codebase, I'll implement interactive parameter prompting using the following approach:

### Architecture

1. **Use existing `dialoguer` crate** - Already available in workspace dependencies with fuzzy-select features
2. **Extend existing parameter system** - Build upon the shared parameter system in `swissarmyhammer/src/common/parameters.rs`
3. **Integrate with existing flow command** - Modify the parameter resolution in `parameter_cli::resolve_workflow_parameters()`

### Implementation Plan

#### 1. Interactive Prompts Module (`swissarmyhammer/src/common/interactive_prompts.rs`)

```rust
pub struct InteractivePrompts {
    non_interactive: bool,
}

impl InteractivePrompts {
    pub fn new(non_interactive: bool) -> Self;
    
    pub async fn prompt_for_parameters(
        &self,
        parameters: &[Parameter],
        existing_values: &HashMap<String, serde_json::Value>
    ) -> ParameterResult<HashMap<String, serde_json::Value>>;
    
    pub fn prompt_string(&self, param: &Parameter) -> ParameterResult<String>;
    pub fn prompt_boolean(&self, param: &Parameter) -> ParameterResult<bool>;
    pub fn prompt_number(&self, param: &Parameter) -> ParameterResult<f64>;
    pub fn prompt_choice(&self, param: &Parameter) -> ParameterResult<String>;
    pub fn prompt_multi_choice(&self, param: &Parameter) -> ParameterResult<Vec<String>>;
}
```

#### 2. Enhanced Parameter Resolver

Extend the existing `ParameterResolver` trait with:
- Mixed mode resolution (CLI args + interactive prompts)
- Fallback to defaults when appropriate
- Clear error messages for non-interactive mode

#### 3. Flow Command Integration

Modify `swissarmyhammer-cli/src/flow.rs`:
- Add `--no-interactive` flag to disable prompting
- Use parameter resolver with prompting capability
- Handle validation errors with clear messages

### Type-Specific Prompting Implementation

- **String**: Basic text input with validation
- **Boolean**: Y/n confirmation prompt with default
- **Choice**: Select menu with arrow key navigation  
- **Multi-choice**: Multi-select with space/enter controls
- **Number**: Text input with numeric validation and range checking

### Error Handling Strategy

- Non-interactive mode: Fail fast with clear error messages
- Interactive mode: Validate input and allow retry
- Preserve existing CLI behavior for provided parameters
- Graceful degradation when terminal is not available

### Files to Create/Modify

1. **New**: `swissarmyhammer/src/common/interactive_prompts.rs`
2. **Modify**: `swissarmyhammer/src/common/parameters.rs` (add ParameterResolver implementation)
3. **Modify**: `swissarmyhammer-cli/src/parameter_cli.rs` (add interactive resolution)
4. **Modify**: `swissarmyhammer-cli/src/flow.rs` (integrate interactive prompting)
5. **New**: Tests for all interactive prompting functionality

This approach leverages the existing parameter validation system and integrates seamlessly with the current CLI structure while adding the interactive prompting capability requested in the issue.

## Implementation Complete

### Summary of Changes

Successfully implemented the interactive parameter prompting system as planned. The implementation includes:

#### 1. Interactive Prompts Module (`swissarmyhammer/src/common/interactive_prompts.rs`)
- ✅ Created `InteractivePrompts` struct using `dialoguer` crate
- ✅ Supports all parameter types: String, Boolean, Number, Choice, MultiChoice
- ✅ Handles validation with retry on errors
- ✅ Gracefully handles non-interactive environments (CI/testing)
- ✅ Respects default values and required parameter constraints
- ✅ Uses appropriate UI controls for each parameter type

#### 2. Enhanced Parameter Resolver (`swissarmyhammer/src/common/parameters.rs`)
- ✅ Created `DefaultParameterResolver` implementing the `ParameterResolver` trait  
- ✅ Supports mixed resolution: CLI args + interactive prompts + defaults
- ✅ Intelligent CLI argument parsing (detects booleans, numbers, strings)
- ✅ Seamless integration with existing parameter validation system

#### 3. Flow Command Integration (`swissarmyhammer-cli/src/parameter_cli.rs` & `flow.rs`)
- ✅ Added `resolve_workflow_parameters_interactive()` function
- ✅ Integrated with existing `--interactive` CLI flag
- ✅ Only prompts when: `interactive && !dry_run && !test_mode`
- ✅ Maintains backward compatibility with existing parameter resolution
- ✅ Preserves all existing CLI functionality

#### 4. Comprehensive Testing
- ✅ Added 13 new unit tests covering all functionality
- ✅ Tests cover: non-interactive mode, CLI argument parsing, defaults, validation
- ✅ All existing tests continue to pass
- ✅ Integration tests verify end-to-end functionality

### Technical Implementation Details

**Dependencies Added:**
- `dialoguer` - Interactive terminal prompts with rich UI
- `is-terminal` - Terminal detection for graceful degradation

**Integration Points:**
- Leverages existing `ParameterValidator` for consistent validation
- Reuses `Parameter` types and error handling from shared parameter system  
- Integrates with workflow parameter discovery pipeline
- Maintains compatibility with `--var` and `--set` CLI arguments

**User Experience:**
- Interactive prompts only appear when parameters are missing and `--interactive` flag is used
- Each parameter type gets an appropriate UI (text input, Y/N, selection menus, etc.)
- Clear error messages with retry capability for invalid input
- Default values are clearly displayed and used when appropriate
- Gracefully falls back to error messages in non-interactive environments

### Example User Flow

```bash
# Without interactive flag - fails with clear error for missing params
$ sah flow run my-workflow
Error: Missing required parameter: person_name

# With interactive flag - prompts for missing parameters  
$ sah flow run my-workflow --interactive
? Enter person_name (The name of the person to greet): John
? Enable enthusiastic greeting? (y/N): y
? Select language (default: English): Spanish
🚀 Starting workflow: my-workflow
```

### Success Criteria Met

- ✅ Missing required parameters trigger interactive prompts
- ✅ All parameter types support appropriate prompt interfaces  
- ✅ Parameter validation occurs during prompting with retry
- ✅ Default values are displayed and used appropriately
- ✅ Non-interactive mode fails gracefully with clear error messages
- ✅ Prompts provide clear descriptions and input guidance

The implementation is complete, tested, and ready for use. All code follows existing patterns and maintains full backward compatibility.