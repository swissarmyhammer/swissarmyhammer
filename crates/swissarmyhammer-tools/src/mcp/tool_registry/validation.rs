//! Base tool helpers plus tool and schema validation types.

use owo_colors::OwoColorize;
use rmcp::model::{CallToolResult, Content};
use rmcp::ErrorData as McpError;
use swissarmyhammer_common::{ErrorSeverity, Severity};

/// Base implementation providing common utility methods for MCP tools
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct BaseToolImpl;

impl BaseToolImpl {
    /// Parse tool arguments from a JSON map into a typed struct
    ///
    /// # Arguments
    ///
    /// * `arguments` - The JSON map of arguments from the MCP request
    ///
    /// # Returns
    ///
    /// * `Result<T, McpError>` - The parsed arguments or an error
    pub fn parse_arguments<T: serde::de::DeserializeOwned>(
        arguments: serde_json::Map<String, serde_json::Value>,
    ) -> std::result::Result<T, McpError> {
        serde_json::from_value(serde_json::Value::Object(arguments))
            .map_err(|e| McpError::invalid_request(format!("invalid arguments: {e}"), None))
    }

    /// Create a success response with serializable content
    ///
    /// # Arguments
    ///
    /// * `content` - The content to include in the response
    ///
    /// # Returns
    ///
    /// * `CallToolResult` - A success response
    pub fn create_success_response<T: Into<String>>(content: T) -> CallToolResult {
        CallToolResult::success(vec![Content::text(content.into())])
    }

    /// Create an error response with the given error message
    ///
    /// # Arguments
    ///
    /// * `error` - The error message
    /// * `details` - Optional additional details
    ///
    /// # Returns
    ///
    /// * `CallToolResult` - An error response
    pub fn create_error_response<T: Into<String>>(
        error: T,
        details: Option<String>,
    ) -> CallToolResult {
        let error_text = match details {
            Some(details) => format!("{}: {}", error.into(), details),
            None => error.into(),
        };

        CallToolResult::error(vec![Content::text(error_text)])
    }
}

/// Errors that can occur during tool validation
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ToolValidationError {
    /// Tool schema validation failed
    SchemaValidation {
        /// Name of the tool that failed validation
        tool_name: String,
        /// The validation error that occurred
        error: ValidationError,
    },

    /// Tool is missing a CLI category but is not hidden from CLI
    MissingCliCategory {
        /// Name of the tool missing a CLI category
        tool_name: String,
    },

    /// Tool has an invalid CLI name
    InvalidCliName {
        /// Name of the tool with invalid CLI name
        tool_name: String,
        /// The invalid CLI name
        cli_name: String,
        /// Reason why the CLI name is invalid
        reason: String,
    },

    /// Tool description is missing or invalid
    InvalidDescription {
        /// Name of the tool with invalid description
        tool_name: String,
        /// Reason why the description is invalid
        reason: String,
    },

    /// Tool name conflicts with another tool
    NameConflict {
        /// Name of the tool with a conflict
        tool_name: String,
        /// Name of the conflicting tool
        conflicting_tool: String,
    },
}

impl std::fmt::Display for ToolValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolValidationError::SchemaValidation { tool_name, error } => {
                write!(
                    f,
                    "schema validation failed for tool '{}': {}",
                    tool_name, error
                )
            }
            ToolValidationError::MissingCliCategory { tool_name } => {
                write!(
                    f,
                    "tool '{}' is visible in CLI but missing CLI category",
                    tool_name
                )
            }
            ToolValidationError::InvalidCliName {
                tool_name,
                cli_name,
                reason,
            } => {
                write!(
                    f,
                    "tool '{}' has invalid CLI name '{}': {}",
                    tool_name, cli_name, reason
                )
            }
            ToolValidationError::InvalidDescription { tool_name, reason } => {
                write!(
                    f,
                    "tool '{}' has invalid description: {}",
                    tool_name, reason
                )
            }
            ToolValidationError::NameConflict {
                tool_name,
                conflicting_tool,
            } => {
                write!(
                    f,
                    "tool '{}' conflicts with existing tool '{}'",
                    tool_name, conflicting_tool
                )
            }
        }
    }
}

impl std::error::Error for ToolValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ToolValidationError::SchemaValidation { error, .. } => Some(error),
            ToolValidationError::MissingCliCategory { .. }
            | ToolValidationError::InvalidCliName { .. }
            | ToolValidationError::InvalidDescription { .. }
            | ToolValidationError::NameConflict { .. } => None,
        }
    }
}

impl Severity for ToolValidationError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: Schema validation failures prevent tool from functioning
            ToolValidationError::SchemaValidation { .. } => ErrorSeverity::Critical,

            // Error: Configuration issues that prevent proper tool operation
            ToolValidationError::MissingCliCategory { .. } => ErrorSeverity::Error,
            ToolValidationError::InvalidCliName { .. } => ErrorSeverity::Error,
            ToolValidationError::InvalidDescription { .. } => ErrorSeverity::Error,
            ToolValidationError::NameConflict { .. } => ErrorSeverity::Error,
        }
    }
}

/// Validation warning for tools
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ToolValidationWarning {
    /// Name of the tool with a validation issue
    pub tool_name: String,
    /// The validation error that occurred
    pub error: ToolValidationError,
    /// Severity level of the validation issue
    pub severity: ToolValidationSeverity,
}

/// Severity levels for tool validation issues
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolValidationSeverity {
    /// Warning level issue that doesn't prevent functionality
    Warning,
    /// Error level issue that may cause problems
    Error,
    /// Critical level issue that will prevent functionality
    Critical,
}

/// Comprehensive validation report for all tools
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolValidationReport {
    /// Total number of tools validated
    pub total_tools: usize,
    /// Number of tools that passed validation
    pub valid_tools: usize,
    /// Number of tools that failed validation
    pub invalid_tools: usize,
    /// List of all validation errors found
    pub errors: Vec<ToolValidationError>,
}

impl ToolValidationReport {
    /// Create a new empty validation report
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if all tools passed validation
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// Generate a summary string of the validation results
    pub fn summary(&self) -> String {
        if self.is_valid() {
            format!("{} All {} tools are valid", "✓".green(), self.total_tools)
        } else {
            format!(
                "{} {} of {} tools have validation errors ({} valid, {} invalid)",
                "✗".red(),
                self.invalid_tools,
                self.total_tools,
                self.valid_tools,
                self.invalid_tools
            )
        }
    }
}

// Import the schema validation types we need
// We need to create a validation module or import from the CLI crate
use serde_json::Value;

// For now, let's define a simple ValidationError type here
// In a real implementation, this would be imported from the schema validation module
/// Errors that can occur during schema validation
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ValidationError {
    /// Schema type is not supported for CLI generation
    UnsupportedSchemaType {
        /// The unsupported schema type
        schema_type: String,
        /// The parameter name where the type was found
        parameter: String,
    },
    /// Schema is invalid or malformed
    InvalidSchema {
        /// Description of the validation failure
        message: String,
    },
    /// Required schema field is missing
    MissingSchemaField {
        /// Name of the missing field
        field: String,
    },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::UnsupportedSchemaType {
                schema_type,
                parameter,
            } => {
                write!(
                    f,
                    "unsupported schema type '{}' for parameter '{}'",
                    schema_type, parameter
                )
            }
            ValidationError::InvalidSchema { message } => {
                write!(f, "invalid schema structure: {}", message)
            }
            ValidationError::MissingSchemaField { field } => {
                write!(f, "missing required schema field: {}", field)
            }
        }
    }
}

impl std::error::Error for ValidationError {}

impl Severity for ValidationError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: Invalid schema structure prevents tool operation
            ValidationError::InvalidSchema { .. } => ErrorSeverity::Critical,

            // Error: Schema validation issues that need fixing
            ValidationError::UnsupportedSchemaType { .. } => ErrorSeverity::Error,
            ValidationError::MissingSchemaField { .. } => ErrorSeverity::Error,
        }
    }
}

// Simple schema validator for the tool registry
// This is a simplified version - in practice, we'd import the full validator
pub(super) struct SchemaValidator;

impl SchemaValidator {
    pub(super) fn validate_schema(schema: &Value) -> Result<(), ValidationError> {
        Self::validate_schema_structure(schema)?;
        Self::validate_properties(schema)?;
        Ok(())
    }

    fn validate_schema_structure(schema: &Value) -> Result<(), ValidationError> {
        if !schema.is_object() {
            return Err(ValidationError::InvalidSchema {
                message: "Schema must be a JSON object".to_string(),
            });
        }

        let schema_obj = schema.as_object().unwrap();
        if !schema_obj.contains_key("properties") {
            return Err(ValidationError::MissingSchemaField {
                field: "properties".to_string(),
            });
        }

        Ok(())
    }

    fn validate_properties(schema: &Value) -> Result<(), ValidationError> {
        let schema_obj = schema.as_object().unwrap();
        let Some(properties) = schema_obj.get("properties").and_then(|p| p.as_object()) else {
            return Err(ValidationError::InvalidSchema {
                message: "'properties' must be a JSON object".to_string(),
            });
        };

        for (prop_name, prop_schema) in properties {
            Self::validate_property_type(prop_name, prop_schema)?;
        }

        Ok(())
    }

    fn validate_property_type(prop_name: &str, prop_schema: &Value) -> Result<(), ValidationError> {
        let Some(prop_obj) = prop_schema.as_object() else {
            return Err(ValidationError::InvalidSchema {
                message: format!("property '{prop_name}' must be a JSON object schema"),
            });
        };
        let Some(prop_type) = prop_obj.get("type").and_then(|t| t.as_str()) else {
            return Ok(());
        };

        match prop_type {
            "string" | "integer" | "number" | "boolean" | "array" => Ok(()),
            unsupported => Err(ValidationError::UnsupportedSchemaType {
                schema_type: unsupported.to_string(),
                parameter: prop_name.to_string(),
            }),
        }
    }
}
