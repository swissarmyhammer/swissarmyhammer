//! Parameter type definitions for prompt templates
//!
//! This module defines the parameter system used by prompts to specify
//! what arguments they expect and how they should be validated.

use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Represents the type of a parameter
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParameterType {
    /// String parameter
    String,
    /// Integer parameter
    Integer,
    /// Boolean parameter
    Boolean,
    /// File path parameter
    File,
    /// Directory path parameter
    Directory,
}

impl ParameterType {
    /// Get the string representation of the parameter type
    pub fn as_str(&self) -> &'static str {
        match self {
            ParameterType::String => "string",
            ParameterType::Integer => "integer", 
            ParameterType::Boolean => "boolean",
            ParameterType::File => "file",
            ParameterType::Directory => "directory",
        }
    }
}

impl FromStr for ParameterType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "string" | "str" => Ok(ParameterType::String),
            "integer" | "int" | "i32" | "i64" => Ok(ParameterType::Integer),
            "boolean" | "bool" => Ok(ParameterType::Boolean),
            "file" => Ok(ParameterType::File),
            "directory" | "dir" => Ok(ParameterType::Directory),
            _ => Err(format!("Unknown parameter type: {}", s)),
        }
    }
}

impl Default for ParameterType {
    fn default() -> Self {
        ParameterType::String
    }
}

/// Represents a parameter specification for a prompt template
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    /// Name of the parameter
    pub name: String,
    /// Description of what this parameter is for
    pub description: String,
    /// Type of the parameter
    pub parameter_type: ParameterType,
    /// Whether this parameter is required
    pub required: bool,
    /// Default value if not provided
    pub default: Option<serde_json::Value>,
    /// Example value for documentation
    pub example: Option<String>,
    /// Valid choices for this parameter (for enum-like parameters)
    pub choices: Option<Vec<String>>,
}

impl Parameter {
    /// Create a new parameter with name, description, and type
    pub fn new(name: impl Into<String>, description: impl Into<String>, parameter_type: ParameterType) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameter_type,
            required: false,
            default: None,
            example: None,
            choices: None,
        }
    }

    /// Mark this parameter as required
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Set a default value for this parameter
    pub fn with_default(mut self, default: serde_json::Value) -> Self {
        self.default = Some(default);
        self
    }

    /// Set an example value for this parameter
    pub fn with_example(mut self, example: impl Into<String>) -> Self {
        self.example = Some(example.into());
        self
    }

    /// Set valid choices for this parameter
    pub fn with_choices(mut self, choices: Vec<String>) -> Self {
        self.choices = Some(choices);
        self
    }
}

/// Trait for types that can provide parameters
pub trait ParameterProvider {
    /// Get the parameters for this type
    fn get_parameters(&self) -> &[Parameter];

    /// Check if all required parameters are provided
    fn validate_parameters(&self, args: &std::collections::HashMap<String, String>) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        
        for param in self.get_parameters() {
            if param.required && !args.contains_key(&param.name) {
                errors.push(format!("Required parameter '{}' is missing", param.name));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parameter_type_from_str() {
        assert_eq!("string".parse::<ParameterType>().unwrap(), ParameterType::String);
        assert_eq!("integer".parse::<ParameterType>().unwrap(), ParameterType::Integer);
        assert_eq!("boolean".parse::<ParameterType>().unwrap(), ParameterType::Boolean);
        assert_eq!("file".parse::<ParameterType>().unwrap(), ParameterType::File);
        assert_eq!("directory".parse::<ParameterType>().unwrap(), ParameterType::Directory);
    }

    #[test]
    fn test_parameter_type_as_str() {
        assert_eq!(ParameterType::String.as_str(), "string");
        assert_eq!(ParameterType::Integer.as_str(), "integer");
        assert_eq!(ParameterType::Boolean.as_str(), "boolean");
        assert_eq!(ParameterType::File.as_str(), "file");
        assert_eq!(ParameterType::Directory.as_str(), "directory");
    }

    #[test]
    fn test_parameter_creation() {
        let param = Parameter::new("name", "User name", ParameterType::String)
            .required(true)
            .with_default(serde_json::Value::String("Anonymous".to_string()))
            .with_example("john_doe");

        assert_eq!(param.name, "name");
        assert_eq!(param.description, "User name");
        assert_eq!(param.parameter_type, ParameterType::String);
        assert!(param.required);
        assert_eq!(param.default, Some(serde_json::Value::String("Anonymous".to_string())));
        assert_eq!(param.example, Some("john_doe".to_string()));
    }
}