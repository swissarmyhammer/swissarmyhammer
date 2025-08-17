# Shared Parameter System Between Prompts and Workflows

**Refer to /Users/wballard/github/sah-parameters/ideas/workflow_parameters.md**

## Objective

Create a unified parameter handling system that can be shared between prompts and workflows to ensure consistent parameter validation, CLI integration, and user experience across the SwissArmyHammer system.

## Current State

- Prompts have their own parameter system with validation
- Workflows use ad-hoc parameter handling
- No shared code between prompt and workflow parameter logic
- Requirement from specification: "make sure to create ONE set of code for parameter handling between prompt and workflow and share it"

## Implementation Tasks

### 1. Extract Common Parameter Types

Create shared parameter types and validation logic:

```rust
// In swissarmyhammer/src/common/parameters.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub parameter_type: ParameterType,
    pub default: Option<serde_json::Value>,
    pub choices: Option<Vec<String>>,
    pub pattern: Option<String>,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParameterType {
    String,
    Boolean,
    Number,
    Choice,
    MultiChoice,
}
```

### 2. Shared Validation Engine

Create common parameter validation logic:

```rust
pub struct ParameterValidator;

impl ParameterValidator {
    pub fn validate_parameter(
        &self,
        param: &Parameter,
        value: &serde_json::Value
    ) -> Result<(), ValidationError>;
    
    pub fn validate_parameters(
        &self,
        params: &[Parameter],
        values: &HashMap<String, serde_json::Value>
    ) -> Result<(), ValidationError>;
}
```

### 3. CLI Integration Traits

Define traits for CLI parameter handling:

```rust
pub trait ParameterProvider {
    fn get_parameters(&self) -> &[Parameter];
    fn validate_context(&self, context: &HashMap<String, serde_json::Value>) -> Result<(), ValidationError>;
}

pub trait ParameterResolver {
    fn resolve_parameters(
        &self,
        parameters: &[Parameter],
        cli_args: &HashMap<String, String>,
        interactive: bool
    ) -> Result<HashMap<String, serde_json::Value>, ValidationError>;
}
```

### 4. Migration Strategy

Update existing systems to use shared parameter code:

- Migrate prompt parameter validation to use shared system
- Update workflow parameter handling to use shared types
- Ensure backward compatibility during transition
- Keep existing API surfaces intact where possible

## Technical Details

### File Structure
```
swissarmyhammer/src/
├── common/
│   ├── parameters.rs      # Shared parameter types and validation
│   └── parameter_cli.rs   # CLI integration helpers
├── prompts/
│   └── mod.rs            # Update to use shared parameters
└── workflow/
    └── mod.rs            # Update to use shared parameters
```

### Integration Points

1. **Prompt System**: Update to use shared `Parameter` type
2. **Workflow System**: Implement `ParameterProvider` trait
3. **CLI Commands**: Use shared `ParameterResolver` for both prompts and workflows
4. **Validation**: Single validation engine for both systems

### Testing Requirements

- Unit tests for shared parameter validation
- Integration tests between prompts and workflows
- Migration tests to ensure no regression
- CLI integration test coverage

## Success Criteria

- [ ] Shared parameter types and validation logic
- [ ] Both prompts and workflows use the same parameter system
- [ ] No code duplication between prompt and workflow parameter handling
- [ ] Backward compatibility maintained
- [ ] Comprehensive test coverage for shared system

## Dependencies

- Requires completion of workflow_parameters_000001_frontmatter-parameter-schema
- Foundation for all subsequent parameter enhancement steps

## Next Steps

After completion, enables:
- Consistent CLI parameter generation for both prompts and workflows
- Unified interactive prompting system
- Shared help text generation
- Consistent validation error messages