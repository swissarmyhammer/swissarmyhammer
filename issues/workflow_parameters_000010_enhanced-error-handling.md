# Enhanced Error Handling and User Experience

**Refer to /Users/wballard/github/sah-parameters/ideas/workflow_parameters.md**

## Objective

Implement comprehensive error handling with clear, actionable error messages that guide users toward correct parameter usage, providing an excellent user experience when parameter validation fails or configuration issues occur.

## Current State

- Basic parameter validation with generic error messages
- Limited context in error messages
- No guided error recovery or suggestions
- Error messages may be technical rather than user-friendly

## Implementation Tasks

### 1. Structured Error Types

Create comprehensive error types that provide detailed context:

```rust
#[derive(Debug, Error)]
pub enum ParameterError {
    #[error("Required parameter '{parameter}' is missing")]
    RequiredParameterMissing { parameter: String },
    
    #[error("Parameter '{parameter}' has invalid value '{value}': {reason}")]
    InvalidParameterValue { 
        parameter: String, 
        value: String, 
        reason: String,
        suggestions: Vec<String>,
    },
    
    #[error("Parameter '{parameter}' must match pattern '{pattern}' (received: '{value}')")]
    PatternMismatch { 
        parameter: String, 
        pattern: String, 
        value: String,
        pattern_description: Option<String>,
        examples: Vec<String>,
    },
    
    #[error("Parameter '{parameter}' must be between {min} and {max} (received: {value})")]
    ValueOutOfRange { 
        parameter: String, 
        min: f64, 
        max: f64, 
        value: f64 
    },
    
    #[error("Parameter '{parameter}' must be one of: {choices} (received: '{value}')")]
    InvalidChoice { 
        parameter: String, 
        choices: String, 
        value: String,
        did_you_mean: Option<String>,
    },
    
    #[error("Parameter '{parameter}' is required because {condition}")]
    ConditionalParameterMissing { 
        parameter: String, 
        condition: String,
        condition_explanation: Option<String>,
    },
    
    #[error("Circular dependency detected in parameter conditions: {cycle}")]
    CircularDependency { cycle: String },
    
    #[error("Parameter '{parameter}' in group '{group}' does not exist in workflow")]
    UnknownParameterInGroup { parameter: String, group: String },
    
    #[error("Invalid condition expression '{expression}': {reason}")]
    InvalidConditionExpression { expression: String, reason: String },
}
```

### 2. User-Friendly Error Messages

Implement error message enhancement with context and suggestions:

```rust
pub struct ErrorMessageEnhancer;

impl ErrorMessageEnhancer {
    pub fn enhance_parameter_error(&self, error: ParameterError) -> EnhancedError {
        match error {
            ParameterError::PatternMismatch { parameter, pattern, value, .. } => {
                let (description, examples) = self.explain_pattern(&pattern);
                
                EnhancedError {
                    message: format!(
                        "Parameter '{}' has invalid format: '{}'", 
                        parameter, value
                    ),
                    explanation: Some(description),
                    examples: if examples.is_empty() { None } else { Some(examples) },
                    suggestions: vec![
                        "Check the format requirements".to_string(),
                        "Use --help to see parameter details".to_string(),
                    ],
                    recoverable: true,
                }
            },
            
            ParameterError::InvalidChoice { parameter, choices, value, .. } => {
                let did_you_mean = self.suggest_closest_match(&value, &choices);
                
                EnhancedError {
                    message: format!(
                        "Parameter '{}' has invalid value: '{}'", 
                        parameter, value
                    ),
                    explanation: Some(format!("Valid options are: {}", choices)),
                    examples: None,
                    suggestions: if let Some(suggestion) = did_you_mean {
                        vec![format!("Did you mean '{}'?", suggestion)]
                    } else {
                        vec!["Choose from the available options".to_string()]
                    },
                    recoverable: true,
                }
            },
            
            ParameterError::ConditionalParameterMissing { parameter, condition, .. } => {
                let explanation = self.explain_condition(&condition);
                
                EnhancedError {
                    message: format!(
                        "Parameter '{}' is required for your current configuration", 
                        parameter
                    ),
                    explanation: Some(explanation),
                    examples: None,
                    suggestions: vec![
                        format!("Provide --{}", parameter.replace('_', "-")),
                        "Use --interactive mode for guided input".to_string(),
                    ],
                    recoverable: true,
                }
            },
            
            _ => EnhancedError::from(error),
        }
    }
    
    fn explain_pattern(&self, pattern: &str) -> (String, Vec<String>) {
        match pattern {
            r"^[^@\s]+@[^@\s]+\.[^@\s]+$" => (
                "Must be a valid email address".to_string(),
                vec![
                    "user@example.com".to_string(),
                    "alice.smith@company.org".to_string(),
                ]
            ),
            r"^https?://[^\s]+$" => (
                "Must be a valid HTTP or HTTPS URL".to_string(),
                vec![
                    "https://example.com".to_string(),
                    "http://localhost:3000".to_string(),
                ]
            ),
            r"^\d+\.\d+\.\d+$" => (
                "Must be a semantic version number".to_string(),
                vec!["1.0.0".to_string(), "2.1.3".to_string()]
            ),
            _ => (
                format!("Must match pattern: {}", pattern),
                vec![]
            ),
        }
    }
    
    fn suggest_closest_match(&self, input: &str, choices: &str) -> Option<String> {
        let choice_list: Vec<&str> = choices.split(", ").collect();
        
        // Simple fuzzy matching - find closest choice
        choice_list.iter()
            .map(|choice| (choice, self.levenshtein_distance(input, choice)))
            .min_by_key(|(_, distance)| *distance)
            .filter(|(_, distance)| *distance <= 2)
            .map(|(choice, _)| choice.to_string())
    }
}

#[derive(Debug)]
pub struct EnhancedError {
    pub message: String,
    pub explanation: Option<String>,
    pub examples: Option<Vec<String>>,
    pub suggestions: Vec<String>,
    pub recoverable: bool,
}
```

### 3. Interactive Error Recovery

Implement error recovery in interactive mode:

```rust
impl InteractivePrompts {
    pub async fn prompt_with_error_recovery(
        &self,
        param: &Parameter
    ) -> Result<serde_json::Value> {
        let mut attempts = 0;
        const MAX_ATTEMPTS: u32 = 3;
        
        loop {
            attempts += 1;
            
            let input = self.get_user_input(param).await?;
            let parsed_value = self.parse_input(param, &input)?;
            
            match ParameterValidator::new().validate_parameter(param, &parsed_value) {
                Ok(_) => return Ok(parsed_value),
                Err(error) => {
                    let enhanced = ErrorMessageEnhancer::new().enhance_parameter_error(error);
                    
                    println!("❌ {}", enhanced.message);
                    
                    if let Some(explanation) = enhanced.explanation {
                        println!("   {}", explanation);
                    }
                    
                    if let Some(examples) = enhanced.examples {
                        println!("   Examples: {}", examples.join(", "));
                    }
                    
                    for suggestion in enhanced.suggestions {
                        println!("💡 {}", suggestion);
                    }
                    
                    if attempts >= MAX_ATTEMPTS {
                        println!("❌ Maximum attempts reached. Use --help for parameter details.");
                        return Err(ParameterError::MaxAttemptsExceeded {
                            parameter: param.name.clone(),
                        });
                    }
                    
                    println!("Please try again ({}/{}):", attempts, MAX_ATTEMPTS);
                }
            }
        }
    }
}
```

### 4. CLI Error Context

Enhance CLI error messages with context and recovery suggestions:

```rust
impl CliErrorHandler {
    pub fn handle_parameter_error(&self, error: ParameterError, workflow_name: &str) -> ! {
        let enhanced = ErrorMessageEnhancer::new().enhance_parameter_error(error);
        
        eprintln!("❌ Workflow parameter error:");
        eprintln!("   {}", enhanced.message);
        
        if let Some(explanation) = enhanced.explanation {
            eprintln!("   {}", explanation);
        }
        
        if !enhanced.suggestions.is_empty() {
            eprintln!("\n💡 Suggestions:");
            for suggestion in enhanced.suggestions {
                eprintln!("   • {}", suggestion);
            }
        }
        
        // Always provide help information
        eprintln!("\n📖 For parameter details, run:");
        eprintln!("   sah flow run {} --help", workflow_name);
        
        if enhanced.recoverable {
            eprintln!("\n🔄 To fix this interactively, run:");
            eprintln!("   sah flow run {} --interactive", workflow_name);
        }
        
        process::exit(2);
    }
}
```

### 5. Validation Error Context

Provide detailed validation context in all scenarios:

```rust
impl ParameterValidator {
    pub fn validate_with_context(
        &self,
        param: &Parameter,
        value: &serde_json::Value,
        context: &ValidationContext
    ) -> Result<(), ParameterError> {
        match param.parameter_type {
            ParameterType::String => {
                let str_value = value.as_str().ok_or_else(|| {
                    ParameterError::InvalidParameterValue {
                        parameter: param.name.clone(),
                        value: value.to_string(),
                        reason: "Expected a text value".to_string(),
                        suggestions: vec!["Provide text in quotes if it contains spaces".to_string()],
                    }
                })?;
                
                self.validate_string_with_context(param, str_value, context)
            },
            
            ParameterType::Number => {
                let num_value = value.as_f64().ok_or_else(|| {
                    ParameterError::InvalidParameterValue {
                        parameter: param.name.clone(),
                        value: value.to_string(),
                        reason: "Expected a numeric value".to_string(),
                        suggestions: vec![
                            "Use numbers without quotes".to_string(),
                            "Examples: 42, 3.14, -10".to_string(),
                        ],
                    }
                })?;
                
                self.validate_number_with_context(param, num_value, context)
            },
            
            // ... other parameter types
        }
    }
    
    fn validate_string_with_context(
        &self,
        param: &Parameter,
        value: &str,
        context: &ValidationContext
    ) -> Result<(), ParameterError> {
        if let Some(rules) = &param.validation {
            // Length validation with context
            if let Some(min_len) = rules.min_length {
                if value.len() < min_len {
                    return Err(ParameterError::InvalidParameterValue {
                        parameter: param.name.clone(),
                        value: value.to_string(),
                        reason: format!("Must be at least {} characters long", min_len),
                        suggestions: vec![
                            format!("Current length: {} characters", value.len()),
                            "Add more characters to meet the minimum requirement".to_string(),
                        ],
                    });
                }
            }
            
            // Pattern validation with enhanced error
            if let Some(pattern) = &rules.pattern {
                let regex = Regex::new(pattern).map_err(|e| {
                    ParameterError::InvalidConditionExpression {
                        expression: pattern.clone(),
                        reason: format!("Invalid regex pattern: {}", e),
                    }
                })?;
                
                if !regex.is_match(value) {
                    let (description, examples) = context.explain_pattern(pattern);
                    return Err(ParameterError::PatternMismatch {
                        parameter: param.name.clone(),
                        pattern: pattern.clone(),
                        value: value.to_string(),
                        pattern_description: Some(description),
                        examples,
                    });
                }
            }
        }
        
        Ok(())
    }
}
```

## Technical Details

### Error Message Guidelines

1. **Clear and Specific**: Exactly what went wrong
2. **Actionable**: What the user can do to fix it
3. **Contextual**: Why the error occurred
4. **Educational**: Help users understand the requirements
5. **Recoverable**: Provide paths to resolution

### File Locations
- `swissarmyhammer/src/common/parameter_errors.rs` - Error types and enhancement
- `swissarmyhammer/src/common/error_recovery.rs` - Interactive error recovery
- `swissarmyhammer-cli/src/error_handling.rs` - CLI error handling
- `swissarmyhammer/src/common/validation_context.rs` - Validation context

### Testing Requirements

- Error message format tests
- Error recovery workflow tests
- Fuzzy matching suggestion tests
- Pattern explanation tests
- CLI error handling tests
- Interactive recovery tests

## Success Criteria

- [ ] Clear, actionable error messages for all parameter validation failures
- [ ] Helpful suggestions and examples in error messages
- [ ] "Did you mean?" suggestions for invalid choices
- [ ] Pattern explanation with common formats (email, URL, etc.)
- [ ] Interactive error recovery with retry attempts
- [ ] CLI error messages include help and recovery information
- [ ] Error messages are tested and consistent

## Dependencies

- Requires completion of workflow_parameters_000002_shared-parameter-system
- Requires completion of workflow_parameters_000004_interactive-parameter-prompting
- Requires completion of workflow_parameters_000005_parameter-validation-rules

## Example Error Messages

### Pattern Mismatch
```
❌ Workflow parameter error:
   Parameter 'email' has invalid format: 'user@domain'

   Must be a valid email address
   Examples: user@example.com, alice.smith@company.org

💡 Suggestions:
   • Check that the email includes a domain extension (.com, .org, etc.)
   • Use --help to see parameter details

📖 For parameter details, run:
   sah flow run deploy --help

🔄 To fix this interactively, run:
   sah flow run deploy --interactive
```

### Invalid Choice with Suggestion
```
❌ Workflow parameter error:
   Parameter 'environment' has invalid value: 'production'

   Valid options are: dev, staging, prod

💡 Suggestions:
   • Did you mean 'prod'?

📖 For parameter details, run:
   sah flow run deploy --help
```

## Next Steps

After completion, enables:
- Excellent user experience with clear guidance
- Reduced support burden through better error messages
- Faster user onboarding and parameter system adoption