//! Shared parameter system for prompts and workflows
//!
//! This module provides unified parameter handling that can be shared between
//! prompts and workflows to ensure consistent parameter validation, CLI integration,
//! and user experience across the SwissArmyHammer system.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use thiserror::Error;

/// Errors that can occur during parameter operations
#[derive(Debug, Error)]
pub enum ParameterError {
    /// Parameter validation failed
    #[error("Parameter validation failed: {message}")]
    ValidationFailed {
        /// Error message describing the validation failure
        message: String,
    },

    /// Required parameter is missing
    #[error("Required parameter '{name}' is missing")]
    MissingRequired {
        /// Name of the missing parameter
        name: String,
    },

    /// Parameter type mismatch
    #[error("Parameter '{name}' expects {expected_type}, got {actual_type}")]
    TypeMismatch {
        /// Name of the parameter with type mismatch
        name: String,
        /// Expected parameter type
        expected_type: String,
        /// Actual parameter type received
        actual_type: String,
    },

    /// Invalid choice value
    #[error("Parameter '{name}' value '{value}' is not in allowed choices: {choices:?}")]
    InvalidChoice {
        /// Name of the parameter with invalid choice
        name: String,
        /// Value that was provided
        value: String,
        /// List of valid choices
        choices: Vec<String>,
    },

    /// Value out of range
    #[error("Parameter '{name}' value {value} is out of range [{min:?}, {max:?}]")]
    OutOfRange {
        /// Name of the parameter with out-of-range value
        name: String,
        /// Value that was provided
        value: f64,
        /// Minimum allowed value
        min: Option<f64>,
        /// Maximum allowed value
        max: Option<f64>,
    },

    /// Pattern validation failed
    #[error("Parameter '{name}' value '{value}' does not match required pattern '{pattern}'")]
    PatternMismatch {
        /// Name of the parameter with invalid format
        name: String,
        /// Value that was provided
        value: String,
        /// Required pattern that the value should match
        pattern: String,
    },
}

/// Result type for parameter operations
pub type ParameterResult<T> = Result<T, ParameterError>;

/// Types of parameters supported by the system
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParameterType {
    /// String text input
    String,
    /// Boolean true/false values  
    Boolean,
    /// Numeric values (integers and floats)
    Number,
    /// Selection from predefined options
    Choice,
    /// Multiple selections from predefined options
    MultiChoice,
}

impl ParameterType {
    /// Get the string representation of this parameter type
    pub fn as_str(&self) -> &'static str {
        match self {
            ParameterType::String => "string",
            ParameterType::Boolean => "boolean",
            ParameterType::Number => "number",
            ParameterType::Choice => "choice",
            ParameterType::MultiChoice => "multi_choice",
        }
    }
}

impl FromStr for ParameterType {
    type Err = (); // We don't want to error on unknown types, just default to String

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let param_type = match s.to_lowercase().as_str() {
            "string" => ParameterType::String,
            "boolean" | "bool" => ParameterType::Boolean,
            "number" | "numeric" | "int" | "integer" | "float" => ParameterType::Number,
            "choice" | "select" => ParameterType::Choice,
            "multi_choice" | "multichoice" | "multiselect" => ParameterType::MultiChoice,
            _ => ParameterType::String, // Default to string for unknown types
        };
        Ok(param_type)
    }
}

/// Unified parameter specification that works for both prompts and workflows
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    /// The parameter name used in templates
    pub name: String,

    /// Human-readable description of the parameter's purpose  
    pub description: String,

    /// Whether this parameter must be provided
    pub required: bool,

    /// The type of parameter value expected
    pub parameter_type: ParameterType,

    /// Default value to use if parameter is not provided
    pub default: Option<serde_json::Value>,

    /// Available choices for Choice and MultiChoice types
    pub choices: Option<Vec<String>>,

    /// Validation pattern (regex) for string parameters
    pub pattern: Option<String>,

    /// Minimum value for number parameters
    pub min: Option<f64>,

    /// Maximum value for number parameters  
    pub max: Option<f64>,
}

impl Parameter {
    /// Create a new parameter with basic information
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameter_type: ParameterType,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            required: false,
            parameter_type,
            default: None,
            choices: None,
            pattern: None,
            min: None,
            max: None,
        }
    }

    /// Create a required parameter
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Set the default value
    pub fn with_default(mut self, default: serde_json::Value) -> Self {
        self.default = Some(default);
        self
    }

    /// Set choices for choice/multichoice parameters  
    pub fn with_choices(mut self, choices: Vec<String>) -> Self {
        self.choices = Some(choices);
        self
    }

    /// Set validation pattern for string parameters
    pub fn with_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.pattern = Some(pattern.into());
        self
    }

    /// Set numeric range constraints
    pub fn with_range(mut self, min: Option<f64>, max: Option<f64>) -> Self {
        self.min = min;
        self.max = max;
        self
    }
}

/// Trait for types that can provide parameters
pub trait ParameterProvider {
    /// Get the parameters defined for this provider
    fn get_parameters(&self) -> &[Parameter];

    /// Validate that the provided context satisfies all parameter requirements
    fn validate_context(
        &self,
        context: &HashMap<String, serde_json::Value>,
    ) -> ParameterResult<()> {
        let validator = ParameterValidator;
        validator.validate_parameters(self.get_parameters(), context)
    }
}

/// Trait for resolving parameters from various sources
pub trait ParameterResolver {
    /// Resolve parameters from CLI arguments and interactive prompting  
    fn resolve_parameters(
        &self,
        parameters: &[Parameter],
        cli_args: &HashMap<String, String>,
        interactive: bool,
    ) -> ParameterResult<HashMap<String, serde_json::Value>>;
}

/// Parameter validation engine
pub struct ParameterValidator;

impl ParameterValidator {
    /// Create a new parameter validator
    pub fn new() -> Self {
        Self
    }

    /// Validate a single parameter value
    pub fn validate_parameter(
        &self,
        param: &Parameter,
        value: &serde_json::Value,
    ) -> ParameterResult<()> {
        // Type validation
        match param.parameter_type {
            ParameterType::String => {
                if !value.is_string() {
                    return Err(ParameterError::TypeMismatch {
                        name: param.name.clone(),
                        expected_type: "string".to_string(),
                        actual_type: self.get_value_type(value),
                    });
                }

                let str_value = value.as_str().unwrap();

                // Pattern validation
                if let Some(pattern) = &param.pattern {
                    if let Ok(regex) = regex::Regex::new(pattern) {
                        if !regex.is_match(str_value) {
                            return Err(ParameterError::PatternMismatch {
                                name: param.name.clone(),
                                value: str_value.to_string(),
                                pattern: pattern.clone(),
                            });
                        }
                    }
                }

                // Choice validation for string parameters with choices
                if let Some(choices) = &param.choices {
                    if !choices.contains(&str_value.to_string()) {
                        return Err(ParameterError::InvalidChoice {
                            name: param.name.clone(),
                            value: str_value.to_string(),
                            choices: choices.clone(),
                        });
                    }
                }
            }

            ParameterType::Boolean => {
                if !value.is_boolean() {
                    return Err(ParameterError::TypeMismatch {
                        name: param.name.clone(),
                        expected_type: "boolean".to_string(),
                        actual_type: self.get_value_type(value),
                    });
                }
            }

            ParameterType::Number => {
                if !value.is_number() {
                    return Err(ParameterError::TypeMismatch {
                        name: param.name.clone(),
                        expected_type: "number".to_string(),
                        actual_type: self.get_value_type(value),
                    });
                }

                let num_value = value.as_f64().unwrap();

                // Range validation
                if let Some(min) = param.min {
                    if num_value < min {
                        return Err(ParameterError::OutOfRange {
                            name: param.name.clone(),
                            value: num_value,
                            min: Some(min),
                            max: param.max,
                        });
                    }
                }

                if let Some(max) = param.max {
                    if num_value > max {
                        return Err(ParameterError::OutOfRange {
                            name: param.name.clone(),
                            value: num_value,
                            min: param.min,
                            max: Some(max),
                        });
                    }
                }
            }

            ParameterType::Choice => {
                if !value.is_string() {
                    return Err(ParameterError::TypeMismatch {
                        name: param.name.clone(),
                        expected_type: "string".to_string(),
                        actual_type: self.get_value_type(value),
                    });
                }

                let str_value = value.as_str().unwrap();

                if let Some(choices) = &param.choices {
                    if !choices.contains(&str_value.to_string()) {
                        return Err(ParameterError::InvalidChoice {
                            name: param.name.clone(),
                            value: str_value.to_string(),
                            choices: choices.clone(),
                        });
                    }
                }
            }

            ParameterType::MultiChoice => {
                if !value.is_array() {
                    return Err(ParameterError::TypeMismatch {
                        name: param.name.clone(),
                        expected_type: "array".to_string(),
                        actual_type: self.get_value_type(value),
                    });
                }

                let array = value.as_array().unwrap();
                if let Some(choices) = &param.choices {
                    for item in array {
                        if let Some(str_item) = item.as_str() {
                            if !choices.contains(&str_item.to_string()) {
                                return Err(ParameterError::InvalidChoice {
                                    name: param.name.clone(),
                                    value: str_item.to_string(),
                                    choices: choices.clone(),
                                });
                            }
                        } else {
                            return Err(ParameterError::TypeMismatch {
                                name: param.name.clone(),
                                expected_type: "array of strings".to_string(),
                                actual_type: "array with non-string items".to_string(),
                            });
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Validate all parameters against provided values
    pub fn validate_parameters(
        &self,
        parameters: &[Parameter],
        values: &HashMap<String, serde_json::Value>,
    ) -> ParameterResult<()> {
        for param in parameters {
            if let Some(value) = values.get(&param.name) {
                // Validate provided value
                self.validate_parameter(param, value)?;
            } else if param.required {
                // Check if parameter is required but not provided
                return Err(ParameterError::MissingRequired {
                    name: param.name.clone(),
                });
            }
        }

        Ok(())
    }

    /// Get the type name of a JSON value
    fn get_value_type(&self, value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::String(_) => "string".to_string(),
            serde_json::Value::Number(_) => "number".to_string(),
            serde_json::Value::Bool(_) => "boolean".to_string(),
            serde_json::Value::Array(_) => "array".to_string(),
            serde_json::Value::Object(_) => "object".to_string(),
            serde_json::Value::Null => "null".to_string(),
        }
    }
}

impl Default for ParameterValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parameter_creation() {
        let param = Parameter::new("test_param", "A test parameter", ParameterType::String)
            .required(true)
            .with_default(serde_json::Value::String("default_value".to_string()));

        assert_eq!(param.name, "test_param");
        assert_eq!(param.description, "A test parameter");
        assert!(param.required);
        assert_eq!(param.parameter_type, ParameterType::String);
        assert_eq!(
            param.default,
            Some(serde_json::Value::String("default_value".to_string()))
        );
    }

    #[test]
    fn test_parameter_type_from_string() {
        assert_eq!(
            "string".parse::<ParameterType>().unwrap(),
            ParameterType::String
        );
        assert_eq!(
            "boolean".parse::<ParameterType>().unwrap(),
            ParameterType::Boolean
        );
        assert_eq!(
            "bool".parse::<ParameterType>().unwrap(),
            ParameterType::Boolean
        );
        assert_eq!(
            "number".parse::<ParameterType>().unwrap(),
            ParameterType::Number
        );
        assert_eq!(
            "choice".parse::<ParameterType>().unwrap(),
            ParameterType::Choice
        );
        assert_eq!(
            "multi_choice".parse::<ParameterType>().unwrap(),
            ParameterType::MultiChoice
        );
        assert_eq!(
            "unknown".parse::<ParameterType>().unwrap(),
            ParameterType::String
        ); // Default
    }

    #[test]
    fn test_parameter_validation_success() {
        let validator = ParameterValidator::new();

        let param = Parameter::new("test", "Test parameter", ParameterType::String).required(true);

        let value = serde_json::Value::String("test_value".to_string());

        assert!(validator.validate_parameter(&param, &value).is_ok());
    }

    #[test]
    fn test_parameter_validation_type_mismatch() {
        let validator = ParameterValidator::new();

        let param = Parameter::new("test", "Test parameter", ParameterType::Boolean);
        let value = serde_json::Value::String("not_a_boolean".to_string());

        let result = validator.validate_parameter(&param, &value);
        assert!(result.is_err());

        if let Err(ParameterError::TypeMismatch {
            name,
            expected_type,
            actual_type,
        }) = result
        {
            assert_eq!(name, "test");
            assert_eq!(expected_type, "boolean");
            assert_eq!(actual_type, "string");
        } else {
            panic!("Expected TypeMismatch error");
        }
    }

    #[test]
    fn test_parameter_validation_missing_required() {
        let validator = ParameterValidator::new();

        let params = vec![Parameter::new(
            "required_param",
            "Required parameter",
            ParameterType::String,
        )
        .required(true)];

        let values = HashMap::new(); // Empty values

        let result = validator.validate_parameters(&params, &values);
        assert!(result.is_err());

        if let Err(ParameterError::MissingRequired { name }) = result {
            assert_eq!(name, "required_param");
        } else {
            panic!("Expected MissingRequired error");
        }
    }

    #[test]
    fn test_parameter_validation_choice() {
        let validator = ParameterValidator::new();

        let param = Parameter::new("choice_param", "Choice parameter", ParameterType::Choice)
            .with_choices(vec!["option1".to_string(), "option2".to_string()]);

        // Valid choice
        let valid_value = serde_json::Value::String("option1".to_string());
        assert!(validator.validate_parameter(&param, &valid_value).is_ok());

        // Invalid choice
        let invalid_value = serde_json::Value::String("invalid_option".to_string());
        let result = validator.validate_parameter(&param, &invalid_value);
        assert!(result.is_err());

        if let Err(ParameterError::InvalidChoice {
            name,
            value,
            choices,
        }) = result
        {
            assert_eq!(name, "choice_param");
            assert_eq!(value, "invalid_option");
            assert_eq!(choices, vec!["option1", "option2"]);
        } else {
            panic!("Expected InvalidChoice error");
        }
    }

    #[test]
    fn test_parameter_validation_number_range() {
        let validator = ParameterValidator::new();

        let param = Parameter::new("number_param", "Number parameter", ParameterType::Number)
            .with_range(Some(1.0), Some(10.0));

        // Valid value
        let valid_value = serde_json::Value::Number(serde_json::Number::from(5));
        assert!(validator.validate_parameter(&param, &valid_value).is_ok());

        // Value below minimum
        let below_min = serde_json::Value::Number(serde_json::Number::from(0));
        let result = validator.validate_parameter(&param, &below_min);
        assert!(result.is_err());

        if let Err(ParameterError::OutOfRange {
            name,
            value,
            min,
            max,
        }) = result
        {
            assert_eq!(name, "number_param");
            assert_eq!(value, 0.0);
            assert_eq!(min, Some(1.0));
            assert_eq!(max, Some(10.0));
        } else {
            panic!("Expected OutOfRange error");
        }
    }
}
