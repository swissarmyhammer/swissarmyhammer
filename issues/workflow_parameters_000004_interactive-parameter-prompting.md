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