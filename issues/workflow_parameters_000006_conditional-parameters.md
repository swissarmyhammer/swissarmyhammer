# Conditional Parameters Implementation

**Refer to /Users/wballard/github/sah-parameters/ideas/workflow_parameters.md**

## Objective

Implement conditional parameters that are only required or displayed when certain conditions are met based on other parameter values, enabling dynamic parameter workflows as described in the specification.

## Current State

- Static parameter definitions with fixed required/optional status
- No support for parameter dependencies or conditions
- All parameters are always prompted/validated regardless of other values

## Implementation Tasks

### 1. Conditional Parameter Schema

Extend parameter definitions to support conditions:

```yaml
parameters:
  - name: deploy_env
    description: Deployment environment
    type: choice
    choices: [dev, staging, prod]
    required: true
    
  - name: prod_confirmation
    description: Confirm production deployment
    type: boolean
    required: true
    condition: "deploy_env == 'prod'"
    
  - name: staging_branch
    description: Branch to deploy to staging
    type: string
    required: true
    condition: "deploy_env == 'staging'"
    default: "develop"
    
  - name: enable_ssl
    description: Enable SSL certificate
    type: boolean
    required: false
    default: false
    
  - name: cert_path
    description: Path to SSL certificate file
    type: string
    required: true
    condition: "enable_ssl == true"
    pattern: '^.*\.(pem|crt)$'
```

### 2. Condition Evaluation Engine

Create a condition evaluation system:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterCondition {
    pub expression: String,          // "deploy_env == 'prod'"
    pub description: Option<String>, // Optional explanation of the condition
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub parameter_type: ParameterType,
    pub default: Option<serde_json::Value>,
    pub choices: Option<Vec<String>>,
    pub validation: Option<ValidationRules>,
    pub condition: Option<ParameterCondition>, // New field
}

pub struct ConditionEvaluator {
    variables: HashMap<String, serde_json::Value>,
}

impl ConditionEvaluator {
    pub fn new(variables: HashMap<String, serde_json::Value>) -> Self {
        Self { variables }
    }
    
    pub fn evaluate(&self, condition: &str) -> Result<bool, ConditionError> {
        // Parse and evaluate condition expression
        // Support: ==, !=, <, >, <=, >=, &&, ||, in, contains
    }
    
    pub fn is_parameter_required(
        &self,
        param: &Parameter,
        context: &HashMap<String, serde_json::Value>
    ) -> Result<bool, ConditionError> {
        if let Some(condition) = &param.condition {
            let mut evaluator = ConditionEvaluator::new(context.clone());
            let condition_met = evaluator.evaluate(&condition.expression)?;
            Ok(param.required && condition_met)
        } else {
            Ok(param.required)
        }
    }
}
```

### 3. Expression Language Support

Implement a simple expression language for conditions:

```rust
pub struct ConditionParser;

impl ConditionParser {
    pub fn parse(&self, expression: &str) -> Result<ConditionAst, ParseError>;
}

#[derive(Debug, Clone)]
pub enum ConditionAst {
    Comparison {
        left: String,           // Parameter name
        operator: ComparisonOp, 
        right: serde_json::Value,
    },
    Logical {
        left: Box<ConditionAst>,
        operator: LogicalOp,
        right: Box<ConditionAst>,
    },
    In {
        parameter: String,
        values: Vec<serde_json::Value>,
    },
    Contains {
        parameter: String,
        substring: String,
    },
}

#[derive(Debug, Clone)]
pub enum ComparisonOp {
    Equal,      // ==
    NotEqual,   // !=  
    Less,       // <
    Greater,    // >
    LessEq,     // <=
    GreaterEq,  // >=
}

#[derive(Debug, Clone)]
pub enum LogicalOp {
    And,        // &&
    Or,         // ||
}
```

### 4. Dynamic Parameter Resolution

Update parameter resolution to handle conditions:

```rust
impl ParameterResolver {
    pub fn resolve_conditional_parameters(
        &self,
        parameters: &[Parameter],
        provided_values: HashMap<String, serde_json::Value>,
        interactive: bool
    ) -> Result<HashMap<String, serde_json::Value>> {
        let mut resolved = provided_values;
        let mut changed = true;
        
        // Iterate until no more parameters become available
        while changed {
            changed = false;
            
            for param in parameters {
                if resolved.contains_key(&param.name) {
                    continue; // Already resolved
                }
                
                // Check if this parameter should be included
                let evaluator = ConditionEvaluator::new(resolved.clone());
                let should_include = if let Some(condition) = &param.condition {
                    evaluator.evaluate(&condition.expression)?
                } else {
                    true // No condition means always include
                };
                
                if should_include {
                    let is_required = evaluator.is_parameter_required(param, &resolved)?;
                    
                    if is_required && interactive {
                        // Prompt for this parameter
                        let value = self.prompt_for_parameter(param).await?;
                        resolved.insert(param.name.clone(), value);
                        changed = true;
                    } else if let Some(default) = &param.default {
                        // Use default value
                        resolved.insert(param.name.clone(), default.clone());
                        changed = true;
                    } else if is_required {
                        return Err(ParameterError::ConditionalParameterMissing {
                            parameter: param.name.clone(),
                            condition: param.condition.as_ref().unwrap().expression.clone(),
                        });
                    }
                }
            }
        }
        
        Ok(resolved)
    }
}
```

### 5. Interactive Prompting with Conditions

Update interactive prompting to handle conditional parameters:

```rust
impl InteractivePrompts {
    pub async fn prompt_parameters_conditionally(
        &self,
        parameters: &[Parameter],
        mut context: HashMap<String, serde_json::Value>
    ) -> Result<HashMap<String, serde_json::Value>> {
        
        // Sort parameters to handle dependencies
        let sorted_params = self.sort_parameters_by_dependencies(parameters)?;
        
        for param in sorted_params {
            let evaluator = ConditionEvaluator::new(context.clone());
            
            // Check if this parameter should be prompted
            let should_prompt = if let Some(condition) = &param.condition {
                match evaluator.evaluate(&condition.expression) {
                    Ok(result) => result,
                    Err(_) => {
                        // Condition references unavailable parameters, skip for now
                        continue;
                    }
                }
            } else {
                true
            };
            
            if should_prompt && !context.contains_key(&param.name) {
                let is_required = evaluator.is_parameter_required(&param, &context)?;
                
                if is_required || self.should_prompt_optional(&param) {
                    println!("\n📋 {}", param.description);
                    if let Some(condition) = &param.condition {
                        println!("   (Shown because: {})", 
                            self.format_condition_explanation(condition));
                    }
                    
                    let value = self.prompt_with_validation(&param).await?;
                    context.insert(param.name.clone(), value);
                }
            }
        }
        
        Ok(context)
    }
    
    fn format_condition_explanation(&self, condition: &ParameterCondition) -> String {
        if let Some(desc) = &condition.description {
            desc.clone()
        } else {
            format!("condition '{}' is met", condition.expression)
        }
    }
}
```

## Technical Details

### Expression Language Features

Support common condition patterns:
- `parameter == "value"` - Exact equality
- `parameter != "value"` - Inequality  
- `parameter in ["a", "b", "c"]` - Value in list
- `parameter > 10` - Numeric comparisons
- `enable_ssl == true && cert_type == "custom"` - Logical operations
- `deploy_env == "prod" || deploy_env == "staging"` - OR conditions

### Dependency Resolution

Handle parameter dependencies:
- Topological sort to determine evaluation order
- Circular dependency detection
- Clear error messages for unsatisfiable conditions

### File Locations
- `swissarmyhammer/src/common/parameter_conditions.rs` - Condition evaluation
- `swissarmyhammer/src/common/condition_parser.rs` - Expression parsing
- `swissarmyhammer/src/common/parameter_resolver.rs` - Updated resolution logic

### Testing Requirements

- Unit tests for condition evaluation engine
- Expression parsing tests with various syntaxes
- Dependency resolution tests
- Interactive prompting with conditions tests
- Error handling for invalid conditions
- Circular dependency detection tests

## Success Criteria

- [ ] Parameters can be conditionally required based on other parameter values
- [ ] Expression language supports common comparison and logical operators
- [ ] Interactive prompting shows/hides parameters based on conditions
- [ ] Clear explanations when conditional parameters appear
- [ ] Dependency resolution handles complex parameter relationships
- [ ] Error messages explain condition evaluation failures
- [ ] No circular dependency issues in parameter definitions

## Dependencies

- Requires completion of workflow_parameters_000002_shared-parameter-system
- Requires completion of workflow_parameters_000004_interactive-parameter-prompting
- Requires completion of workflow_parameters_000005_parameter-validation-rules

## Example User Experience

```bash
$ sah flow run deploy
? Select deploy_env: 
  > dev
    staging
    prod

# User selects "prod"

? Confirm production deployment (required because deploy_env is 'prod'): (y/N): y

# User selects "staging" instead
? Select deploy_env: staging
? Enter staging_branch (Branch to deploy to staging) [develop]: feature/new-ui

🚀 Starting workflow: deploy
```

## Next Steps

After completion, enables:
- Parameter groups for better organization
- Enhanced CLI help with conditional parameter documentation
- Complex multi-step parameter workflows