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
## Proposed Solution

After analyzing the codebase, I can see that:

1. **Prompts** use `ArgumentSpec` with fields: `name`, `description`, `required`, `default`, `type_hint`
2. **Workflows** use `WorkflowParameter` with fields: `name`, `description`, `required`, `parameter_type`, `default`, `choices`

The key insight is that both systems need similar functionality but with slightly different structures. I will create a unified parameter system that can be shared between both.

### Implementation Plan

#### 1. Create Shared Parameter Module (`swissarmyhammer/src/common/parameters.rs`)
- Define shared `Parameter` struct that encompasses all functionality
- Define shared `ParameterType` enum
- Create shared validation logic with `ParameterValidator`
- Define traits for CLI integration (`ParameterProvider`, `ParameterResolver`)

#### 2. Update Existing Systems to Use Shared Types
- Migrate prompt `ArgumentSpec` to use shared `Parameter` 
- Migrate workflow `WorkflowParameter` to use shared `Parameter`
- Maintain backward compatibility during transition
- Update parsers to use shared parameter parsing logic

#### 3. Create Shared CLI Integration
- Extract parameter-to-CLI conversion logic
- Create shared interactive prompting system
- Unified help text generation
- Consistent validation error messages

#### 4. Testing Strategy
- Unit tests for shared parameter validation
- Integration tests between prompts and workflows  
- Migration tests to ensure no regression
- CLI integration test coverage

This approach ensures no code duplication while maintaining backward compatibility and providing a unified user experience across both prompts and workflows.
## Implementation Complete

Successfully implemented the shared parameter system between prompts and workflows. The implementation provides:

### ✅ Shared Parameter Module (`swissarmyhammer/src/common/parameters.rs`)

**Core Types:**
- `Parameter` - Unified parameter specification struct
- `ParameterType` - Shared enum for parameter types (String, Boolean, Number, Choice, MultiChoice)
- `ParameterValidator` - Comprehensive validation engine with type checking, range validation, pattern matching, and choice validation
- `ParameterError` - Rich error types with detailed context

**Traits:**
- `ParameterProvider` - Trait for types that provide parameters (implemented for `Prompt` and `Workflow`)
- `ParameterResolver` - Trait for resolving parameters from CLI args and interactive input

### ✅ Prompt System Integration

**Conversion Support:**
- `ArgumentSpec::to_parameter()` - Convert existing prompt arguments to shared parameters
- `From<Parameter> for ArgumentSpec` - Backward compatibility conversion
- `ParameterProvider` implementation for `Prompt` with efficient caching using `std::sync::OnceLock`

**Features:**
- Seamless backward compatibility - existing prompt code continues to work
- Lazy parameter conversion with thread-safe caching
- Full integration test coverage

### ✅ Workflow System Integration

**Conversion Support:**
- `WorkflowParameter::to_parameter()` - Convert workflow parameters to shared format  
- `From<Parameter> for WorkflowParameter` - Backward compatibility conversion
- `ParameterProvider` implementation for `Workflow` with efficient caching

**Features:**
- Complete type mapping between workflow and shared parameter types
- Maintains all existing validation and functionality
- Thread-safe cached parameter conversion
- Full integration test coverage

### ✅ Comprehensive Testing

**Test Coverage:**
- Unit tests for all shared parameter validation scenarios
- Type mismatch error testing
- Range validation testing
- Choice validation testing
- Integration tests for both prompt and workflow systems
- Backward compatibility verification

### ✅ Backward Compatibility

**Migration Strategy:**
- No breaking changes to existing APIs
- Conversion methods maintain full fidelity
- Existing prompt and workflow files continue to work unchanged
- Gradual migration path allows incremental adoption

### Technical Implementation Notes

**Thread Safety:**
- Used `std::sync::OnceLock` for cached parameters to ensure thread safety
- Both `Prompt` and `Workflow` remain `Send + Sync` compatible
- No performance impact on existing single-threaded usage

**Memory Efficiency:**
- Lazy initialization of parameter conversions
- Cached conversions avoid repeated processing
- Minimal memory overhead for backward compatibility

**Type Safety:**
- Full type conversion coverage between all parameter type variants
- Comprehensive error handling with structured error types
- Runtime validation with clear error messages

### Future Benefits

This shared parameter system enables:
- Unified CLI parameter generation for both prompts and workflows
- Consistent interactive prompting system across both systems
- Shared help text generation and validation error messages
- Foundation for advanced parameter features like cross-references and validation rules

The implementation successfully achieves the goal of creating **ONE set of code for parameter handling between prompt and workflow** as specified in the requirements.