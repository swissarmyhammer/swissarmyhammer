# Builtin Workflow Migration to Parameter Format

**Refer to /Users/wballard/github/sah-parameters/ideas/workflow_parameters.md**

## Objective

Migrate existing builtin workflows (greeting.md, plan.md) from ad-hoc parameter documentation to the new structured parameter format using YAML frontmatter, ensuring backward compatibility and improved user experience.

## Current State

Existing builtin workflows use:
- Ad-hoc `## Parameters` sections in markdown content
- Manual documentation of parameter requirements
- No formal parameter validation or type checking
- Liquid template variables without structured definitions

## Implementation Tasks

### 1. Analyze Existing Workflows

Document current parameter usage in builtin workflows:

#### builtin/workflows/greeting.md
Current parameters (from liquid templates):
- `{{ person_name }}` - Name of person to greet
- `{{ language }}` - Language for greeting
- `{{ enthusiastic }}` - Whether to use enthusiastic greeting

#### builtin/workflows/plan.md  
Current parameters (from specification):
- `plan_filename` - Path to specification file to process

### 2. Convert greeting.md to New Format

Transform greeting workflow to use structured parameters:

```yaml
---
title: Greeting Workflow
description: A workflow that greets someone in different languages
parameters:
  - name: person_name
    description: The name of the person to greet
    required: true
    type: string
    
  - name: language
    description: The language to use for greeting
    required: false
    type: choice
    default: English
    choices:
      - English
      - Spanish
      - French
      - German
      - Italian
      
  - name: enthusiastic
    description: Whether to use enthusiastic greeting
    required: false
    type: boolean
    default: false
---

# Greeting Workflow

This workflow generates personalized greetings in multiple languages.

## Actions

The workflow will:
1. Generate appropriate greeting based on selected language
2. Personalize greeting with provided name  
3. Apply enthusiastic formatting if requested

## Usage

All parameters can be provided via CLI switches or interactive prompting:

```bash
# CLI switches
sah flow run greeting --person-name "Alice" --language "Spanish" --enthusiastic

# Interactive mode
sah flow run greeting --interactive
```
```

### 3. Convert plan.md to New Format

Transform plan workflow to use structured parameters:

```yaml
---
title: Planning Workflow
description: Turn specifications into multiple step plans
parameters:
  - name: plan_filename
    description: Path to the specification file to process
    required: true
    type: string
    pattern: '^.*\.md$'
    
parameter_groups:
  - name: input
    description: Specification input configuration
    parameters: [plan_filename]
---

# Planning Workflow

This workflow processes specification files and generates detailed implementation plans.

## Actions

The workflow will:
1. Read and analyze the specified plan file
2. Break down requirements into implementable steps
3. Generate ordered issue files with detailed tasks
4. Provide implementation guidance and context

## Usage

Provide the path to your specification file:

```bash
# CLI switch
sah flow run plan --plan-filename "./specification/my-feature.md"

# Interactive mode
sah flow run plan --interactive
```
```

### 4. Maintain Backward Compatibility

Ensure existing usage patterns continue to work:

```rust
impl WorkflowParameterResolver {
    pub fn resolve_with_backward_compatibility(
        &self,
        workflow: &Workflow,
        cli_args: &HashMap<String, String>,
        vars: &[String],
        set_vars: &[String]
    ) -> Result<HashMap<String, serde_json::Value>> {
        let mut resolved = HashMap::new();
        
        // 1. First, resolve new-style parameter switches
        for param in workflow.get_parameters() {
            let switch_name = param.to_cli_switch().trim_start_matches("--");
            if let Some(value) = cli_args.get(switch_name) {
                resolved.insert(param.name.clone(), 
                    self.parse_parameter_value(param, value)?);
            }
        }
        
        // 2. Then, handle legacy --var arguments
        for var in vars {
            let parts: Vec<&str> = var.splitn(2, '=').collect();
            if parts.len() == 2 {
                let key = parts[0].to_string();
                let value = serde_json::Value::String(parts[1].to_string());
                
                // Only add if not already resolved via parameter switch
                if !resolved.contains_key(&key) {
                    resolved.insert(key, value);
                }
            }
        }
        
        // 3. Finally, handle legacy --set arguments (for liquid templates)
        let mut set_variables = HashMap::new();
        for set_var in set_vars {
            let parts: Vec<&str> = set_var.splitn(2, '=').collect();
            if parts.len() == 2 {
                set_variables.insert(
                    parts[0].to_string(),
                    serde_json::Value::String(parts[1].to_string())
                );
            }
        }
        
        if !set_variables.is_empty() {
            resolved.insert(
                "_template_vars".to_string(),
                serde_json::to_value(set_variables)?
            );
        }
        
        Ok(resolved)
    }
}
```

### 5. Update Documentation

Update markdown content to reference new parameter system:

- Remove manual `## Parameters` sections
- Update usage examples to show both CLI switches and interactive mode
- Add parameter validation information
- Include examples of different parameter types

### 6. Testing Migration

Comprehensive testing of migrated workflows:

```rust
#[cfg(test)]
mod workflow_migration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_greeting_workflow_parameter_migration() {
        let workflow = WorkflowStorage::file_system()
            .unwrap()
            .get_workflow(&WorkflowName::new("greeting"))
            .unwrap();
            
        // Test new parameter structure
        assert_eq!(workflow.get_parameters().len(), 3);
        
        let person_name_param = workflow.get_parameter("person_name").unwrap();
        assert!(person_name_param.required);
        assert_eq!(person_name_param.parameter_type, ParameterType::String);
        
        let language_param = workflow.get_parameter("language").unwrap();
        assert!(!language_param.required);
        assert_eq!(language_param.parameter_type, ParameterType::Choice);
        assert_eq!(language_param.choices.as_ref().unwrap().len(), 5);
    }
    
    #[tokio::test] 
    async fn test_greeting_backward_compatibility() {
        // Test that old-style --var arguments still work
        let result = execute_workflow_with_args(
            "greeting",
            vec!["--var", "person_name=John", "--var", "language=Spanish"]
        ).await;
        
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_plan_workflow_parameter_migration() {
        let workflow = WorkflowStorage::file_system()
            .unwrap()
            .get_workflow(&WorkflowName::new("plan"))
            .unwrap();
            
        // Test new parameter structure
        assert_eq!(workflow.get_parameters().len(), 1);
        
        let plan_filename_param = workflow.get_parameter("plan_filename").unwrap();
        assert!(plan_filename_param.required);
        assert_eq!(plan_filename_param.parameter_type, ParameterType::String);
        
        // Test pattern validation for .md files
        assert!(plan_filename_param.validation.is_some());
        let validation = plan_filename_param.validation.as_ref().unwrap();
        assert_eq!(validation.pattern.as_ref().unwrap(), "^.*\\.md$");
    }
}
```

## Technical Details

### Migration Strategy

1. **Gradual Migration**: Update workflows one at a time
2. **Backward Compatibility**: Maintain support for existing usage patterns  
3. **Enhanced Features**: Add validation and better error messages
4. **Testing**: Comprehensive testing to ensure no regressions

### File Changes

Files to be modified:
- `builtin/workflows/greeting.md` - Convert to structured parameters
- `builtin/workflows/plan.md` - Convert to structured parameters
- Add migration tests to ensure compatibility

### Parameter Mappings

#### greeting.md Migration
- `person_name`: string, required
- `language`: choice (English, Spanish, French, German, Italian), default: English  
- `enthusiastic`: boolean, default: false

#### plan.md Migration
- `plan_filename`: string, required, pattern: `^.*\.md$`

### Testing Requirements

- Migration tests for each builtin workflow
- Backward compatibility tests with legacy arguments
- Parameter validation tests
- Interactive prompting tests
- CLI help generation tests
- Integration tests with real workflow execution

## Success Criteria

- [ ] All builtin workflows use structured parameter format
- [ ] CLI switches generated from parameter definitions
- [ ] Interactive prompting works for all builtin workflows  
- [ ] Backward compatibility maintained for existing usage
- [ ] Parameter validation provides helpful error messages
- [ ] Help text shows organized parameter information
- [ ] Migration tests pass for all scenarios

## Dependencies

- Requires completion of workflow_parameters_000001_frontmatter-parameter-schema
- Requires completion of workflow_parameters_000002_shared-parameter-system
- Requires completion of workflow_parameters_000003_cli-parameter-switches
- Requires completion of workflow_parameters_000004_interactive-parameter-prompting

## Example Before/After

### Before (greeting.md)
```markdown
# Greeting Workflow

## Parameters

- `person_name`: The name of the person to greet
- `language`: The language to use (English, Spanish, French)
- `enthusiastic`: Whether to use enthusiastic greeting

## Usage

sah flow run greeting --set person_name=John --set language=Spanish
```

### After (greeting.md) 
```yaml
---
title: Greeting Workflow
description: Generate personalized greetings in multiple languages
parameters:
  - name: person_name
    description: The name of the person to greet
    required: true
    type: string
  # ... more parameters
---

# Greeting Workflow

Usage:
sah flow run greeting --person-name "John" --language "Spanish" --enthusiastic
```

## Next Steps

After completion, enables:
- Comprehensive testing of the complete parameter system
- Documentation updates with examples
- Enhanced error handling and user experience