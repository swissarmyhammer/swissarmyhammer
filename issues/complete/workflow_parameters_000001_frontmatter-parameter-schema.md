# Frontmatter Parameter Schema Implementation

**Refer to /Users/wballard/github/sah-parameters/ideas/workflow_parameters.md**

## Objective

Extend workflow frontmatter parsing to support parameter definitions using the same structure as prompts, enabling structured parameter schema definition in YAML frontmatter.

## Current State

Workflows currently support:
- Basic frontmatter with title and description
- Ad-hoc parameter documentation in markdown
- Liquid template variables without formal validation

## Implementation Tasks

### 1. Extend Frontmatter Data Structure

Update workflow frontmatter parsing to include a `parameters` field:

```yaml
---
title: Greeting Workflow
description: A workflow that greets someone
parameters:
  - name: person_name
    description: The name of the person to greet
    required: true
    type: string
  - name: language
    description: The language to use for greeting
    required: false
    type: string
    default: English
    choices:
      - English
      - Spanish
      - French
---
```

### 2. Parameter Type Definitions

Support parameter types consistent with prompts:
- `string` - Text input
- `boolean` - True/false values  
- `number` - Numeric values
- `choice` - Selection from predefined options
- `multi_choice` - Multiple selections from options

### 3. Parser Updates

- Extend `parse_front_matter()` function to handle `parameters` field
- Add parameter validation during workflow loading
- Ensure parameter schema is stored in workflow metadata
- Maintain backward compatibility with existing workflows

### 4. Validation Framework

- Create parameter validation logic
- Support required vs optional parameters
- Validate parameter values against type constraints
- Provide clear error messages for validation failures

## Technical Details

### File Locations
- `swissarmyhammer/src/workflow/mod.rs` - Core workflow structures
- `swissarmyhammer/src/workflow/parser.rs` - Frontmatter parsing
- `swissarmyhammer/src/workflow/validation.rs` - Parameter validation

### Data Structures

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowParameter {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub parameter_type: ParameterType,
    pub default: Option<serde_json::Value>,
    pub choices: Option<Vec<String>>,
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

### Testing Requirements

- Unit tests for frontmatter parsing with parameters
- Parameter validation test cases
- Backward compatibility tests
- Error condition testing

## Success Criteria

- [ ] Workflow frontmatter supports parameter definitions
- [ ] Parameter schema validation during workflow loading
- [ ] Backward compatibility maintained for existing workflows  
- [ ] Comprehensive test coverage
- [ ] Error messages provide clear guidance

## Next Steps

After completion, this enables:
- CLI parameter switch generation (step 2)
- Interactive parameter prompting (step 3)
- Parameter help text generation (step 4)

## Proposed Solution

After analyzing the existing codebase, I see that:

1. **Workflow Structure**: The `Workflow` struct in `definition.rs` contains metadata via `HashMap<String, String>` but no dedicated parameter schema support.

2. **Parser Implementation**: The `MermaidParser` in `parser.rs` extracts frontmatter using `parse_front_matter()` but currently doesn't handle a `parameters` field.

3. **Prompt Parameter Consistency**: The prompt system uses `ArgumentSpec` with fields: `name`, `description`, `required`, `default`, and `type_hint` (as String). This gives us a good model to follow.

### Implementation Plan

1. **Create Parameter Data Structures** (in `workflow/definition.rs`):
   - `WorkflowParameter` struct similar to prompt's `ArgumentSpec`
   - `ParameterType` enum for type validation
   - Add `parameters: Vec<WorkflowParameter>` field to `Workflow` struct

2. **Extend Parser** (in `workflow/parser.rs`):
   - Update frontmatter extraction to parse `parameters` field  
   - Add parameter parsing logic in `parse_front_matter()`
   - Store parsed parameters in workflow metadata initially

3. **Implement Validation**:
   - Parameter type validation during workflow loading
   - Required vs optional parameter checking
   - Type constraint validation (choice options, etc.)

4. **Maintain Backward Compatibility**:
   - Make parameters field optional in frontmatter
   - Existing workflows continue working unchanged
   - Graceful handling of missing parameters

### Data Structure Design

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowParameter {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub parameter_type: ParameterType,
    pub default: Option<serde_json::Value>,
    pub choices: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParameterType {
    String,
    Boolean,
    Number,
    Choice,        // Single choice from options
    MultiChoice,   // Multiple choices from options  
}
```

This matches the issue specification while being consistent with the prompt system's approach to parameter handling.

## Implementation Complete ✅

I have successfully implemented frontmatter parameter schema support for workflows! Here's what was accomplished:

### ✅ Data Structures Added

**New Types in `workflow/definition.rs`:**
- `ParameterType` enum supporting `String`, `Boolean`, `Number`, `Choice`, `MultiChoice`
- `WorkflowParameter` struct with fields: `name`, `description`, `required`, `parameter_type`, `default`, `choices`
- Extended `Workflow` struct with `parameters: Vec<WorkflowParameter>` field

### ✅ Frontmatter Parsing Extended

**Enhanced `workflow/parser.rs`:**
- Added `extract_parameters_from_frontmatter()` method to parse YAML parameter definitions
- Updated `parse_with_metadata()` to extract and store parameters
- Created new `convert_state_diagram_with_actions_metadata_and_parameters()` method
- Maintained full backward compatibility with existing workflows

### ✅ Parameter Validation Implemented

**Comprehensive validation in `workflow/definition.rs`:**
- `validate_parameters()` method validates parameter schemas
- Checks for empty names/descriptions, duplicate names
- Validates choice constraints for `Choice`/`MultiChoice` types  
- Validates default value types match parameter types
- String parameters can optionally have choices for UI hints
- Integrated with main workflow validation via `validate_structure()`

### ✅ Comprehensive Test Coverage

**Added 16+ new tests covering:**
- Parameter parsing from frontmatter with all parameter types
- Workflows with and without parameters
- Backward compatibility with existing workflows  
- Parameter validation error conditions
- Invalid parameter configurations
- Edge cases (empty frontmatter, unknown types, etc.)

### ✅ Example Usage

The implementation now supports frontmatter like this:

```yaml
---
title: Greeting Workflow
description: A workflow that greets someone
parameters:
  - name: person_name
    description: The name of the person to greet
    required: true
    type: string
  - name: language
    description: The language to use for greeting
    required: false
    type: string
    default: English
    choices:
      - English
      - Spanish
      - French
  - name: formal
    description: Use formal greeting
    required: false
    type: boolean
    default: false
---
```

### ✅ Success Criteria Met

- [x] Workflow frontmatter supports parameter definitions
- [x] Parameter schema validation during workflow loading  
- [x] Backward compatibility maintained for existing workflows
- [x] Comprehensive test coverage (16 new tests, all passing)
- [x] Error messages provide clear guidance

### Technical Notes

- Parameters are stored in the `Workflow.parameters` field as `Vec<WorkflowParameter>`
- All parameter types from the specification are supported
- String parameters can have optional choices for UI dropdown hints
- Choice/MultiChoice parameters must have choices defined
- Default values are type-validated against parameter types
- Parsing gracefully handles missing parameters field (empty Vec)
- Full integration with existing workflow validation framework

This foundation enables the next steps: CLI parameter switches, interactive prompting, and parameter help text generation.