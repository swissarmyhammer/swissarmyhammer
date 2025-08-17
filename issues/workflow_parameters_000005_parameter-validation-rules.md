# Advanced Parameter Validation Rules

**Refer to /Users/wballard/github/sah-parameters/ideas/workflow_parameters.md**

## Objective

Implement advanced parameter validation rules including patterns, ranges, and custom validation expressions to provide comprehensive parameter validation for workflows, matching the enhanced features described in the specification.

## Current State

- Basic parameter type validation (string, boolean, number, choice)
- Required vs optional parameter checking
- Simple default value support
- No advanced validation constraints

## Implementation Tasks

### 1. Extended Parameter Schema

Extend the parameter definition to support validation rules:

```yaml
parameters:
  - name: email
    type: string
    pattern: '^[^@]+@[^@]+\.[^@]+$'
    description: Valid email address
    
  - name: port
    type: number
    min: 1
    max: 65535
    description: Network port number
    
  - name: password
    type: string
    min_length: 8
    max_length: 128
    description: Secure password
    
  - name: percentage
    type: number
    min: 0.0
    max: 100.0
    description: Percentage value (0-100)
```

### 2. Validation Rule Types

Implement validation for different constraint types:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRules {
    // String validation
    pub pattern: Option<String>,           // Regex pattern
    pub min_length: Option<usize>,         // Minimum string length
    pub max_length: Option<usize>,         // Maximum string length
    
    // Number validation  
    pub min: Option<f64>,                  // Minimum numeric value
    pub max: Option<f64>,                  // Maximum numeric value
    pub step: Option<f64>,                 // Numeric step/increment
    
    // Choice validation
    pub allow_custom: Option<bool>,        // Allow values not in choices list
    
    // Multi-choice validation
    pub min_selections: Option<usize>,     // Minimum number of selections
    pub max_selections: Option<usize>,     // Maximum number of selections
    
    // Custom validation
    pub custom_validator: Option<String>,  // Custom validation expression
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
}
```

### 3. Validation Engine Enhancement

Extend the validation engine to handle advanced rules:

```rust
impl ParameterValidator {
    pub fn validate_string_parameter(
        &self,
        param: &Parameter,
        value: &str
    ) -> Result<(), ValidationError> {
        if let Some(rules) = &param.validation {
            // Pattern validation
            if let Some(pattern) = &rules.pattern {
                let regex = Regex::new(pattern).map_err(|e| {
                    ValidationError::invalid_pattern(pattern, e.to_string())
                })?;
                
                if !regex.is_match(value) {
                    return Err(ValidationError::pattern_mismatch(
                        &param.name, pattern, value
                    ));
                }
            }
            
            // Length validation
            if let Some(min_len) = rules.min_length {
                if value.len() < min_len {
                    return Err(ValidationError::string_too_short(
                        &param.name, min_len, value.len()
                    ));
                }
            }
            
            if let Some(max_len) = rules.max_length {
                if value.len() > max_len {
                    return Err(ValidationError::string_too_long(
                        &param.name, max_len, value.len()
                    ));
                }
            }
        }
        
        Ok(())
    }
    
    pub fn validate_number_parameter(
        &self,
        param: &Parameter,
        value: f64
    ) -> Result<(), ValidationError> {
        if let Some(rules) = &param.validation {
            // Range validation
            if let Some(min) = rules.min {
                if value < min {
                    return Err(ValidationError::number_too_small(
                        &param.name, min, value
                    ));
                }
            }
            
            if let Some(max) = rules.max {
                if value > max {
                    return Err(ValidationError::number_too_large(
                        &param.name, max, value
                    ));
                }
            }
            
            // Step validation
            if let Some(step) = rules.step {
                if (value % step).abs() > f64::EPSILON {
                    return Err(ValidationError::invalid_step(
                        &param.name, step, value
                    ));
                }
            }
        }
        
        Ok(())
    }
}
```

### 4. Enhanced Error Messages

Provide detailed, user-friendly error messages:

```rust
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Parameter '{param}' must match pattern '{pattern}' (got: '{value}')")]
    PatternMismatch { param: String, pattern: String, value: String },
    
    #[error("Parameter '{param}' must be at least {min} characters long (got: {actual})")]
    StringTooShort { param: String, min: usize, actual: usize },
    
    #[error("Parameter '{param}' must be between {min} and {max} (got: {actual})")]
    NumberOutOfRange { param: String, min: f64, max: f64, actual: f64 },
    
    #[error("Parameter '{param}' must be a multiple of {step} (got: {actual})")]
    InvalidStep { param: String, step: f64, actual: f64 },
    
    #[error("Parameter '{param}' must select between {min} and {max} options (got: {actual})")]
    InvalidSelectionCount { param: String, min: usize, max: usize, actual: usize },
}
```

### 5. Interactive Prompt Integration

Integrate validation into interactive prompting:

```rust
impl InteractivePrompts {
    pub async fn prompt_with_validation(
        &self,
        param: &Parameter
    ) -> Result<serde_json::Value> {
        loop {
            let input = self.get_user_input(param).await?;
            let parsed_value = self.parse_input(param, &input)?;
            
            match ParameterValidator::new().validate_parameter(param, &parsed_value) {
                Ok(_) => return Ok(parsed_value),
                Err(error) => {
                    println!("❌ {}", error);
                    
                    // Provide helpful hints based on validation rules
                    if let Some(rules) = &param.validation {
                        self.print_validation_hints(param, rules);
                    }
                    
                    println!("Please try again.");
                }
            }
        }
    }
    
    fn print_validation_hints(&self, param: &Parameter, rules: &ValidationRules) {
        match param.parameter_type {
            ParameterType::String => {
                if let Some(pattern) = &rules.pattern {
                    println!("💡 Expected format: {}", self.pattern_hint(pattern));
                }
                if let (Some(min), Some(max)) = (rules.min_length, rules.max_length) {
                    println!("💡 Length must be between {} and {} characters", min, max);
                }
            },
            ParameterType::Number => {
                if let (Some(min), Some(max)) = (rules.min, rules.max) {
                    println!("💡 Value must be between {} and {}", min, max);
                }
            },
            _ => {}
        }
    }
}
```

## Technical Details

### Pattern Validation

Use `regex` crate for pattern validation with common patterns:

```rust
pub struct CommonPatterns;

impl CommonPatterns {
    pub const EMAIL: &'static str = r"^[^@\s]+@[^@\s]+\.[^@\s]+$";
    pub const URL: &'static str = r"^https?://[^\s]+$";
    pub const IPV4: &'static str = r"^(\d{1,3}\.){3}\d{1,3}$";
    pub const SEMVER: &'static str = r"^\d+\.\d+\.\d+$";
    
    pub fn hint_for_pattern(pattern: &str) -> &'static str {
        match pattern {
            Self::EMAIL => "example@domain.com",
            Self::URL => "https://example.com",
            Self::IPV4 => "192.168.1.1", 
            Self::SEMVER => "1.2.3",
            _ => pattern,
        }
    }
}
```

### File Locations
- `swissarmyhammer/src/common/parameter_validation.rs` - Validation engine
- `swissarmyhammer/src/common/validation_rules.rs` - Validation rule types
- `swissarmyhammer/src/common/validation_errors.rs` - Error types and messages

### Testing Requirements

- Unit tests for each validation rule type
- Pattern validation tests with valid/invalid inputs
- Range validation tests for numbers
- Length validation tests for strings  
- Multi-choice selection count tests
- Error message format tests
- Interactive prompting with validation tests

## Success Criteria

- [ ] String pattern validation with regex support
- [ ] Number range validation (min/max/step)
- [ ] String length validation (min_length/max_length)
- [ ] Multi-choice selection count validation
- [ ] Clear, actionable error messages for validation failures
- [ ] Interactive prompts include validation hints
- [ ] Common pattern presets for email, URL, IP addresses
- [ ] Comprehensive test coverage for all validation rules

## Dependencies

- Requires completion of workflow_parameters_000002_shared-parameter-system
- Requires completion of workflow_parameters_000004_interactive-parameter-prompting

## Example Usage

```yaml
parameters:
  - name: email
    description: Your email address  
    type: string
    required: true
    pattern: '^[^@\s]+@[^@\s]+\.[^@\s]+$'
    
  - name: retry_count
    description: Number of retry attempts
    type: number
    required: false
    default: 3
    min: 1
    max: 10
    
  - name: tags
    description: Select applicable tags
    type: multi_choice
    choices: ["urgent", "bug", "feature", "docs"]
    min_selections: 1
    max_selections: 3
```

## Next Steps

After completion, enables:
- Conditional parameters based on other parameter values
- Parameter groups and organization
- Custom validation expressions