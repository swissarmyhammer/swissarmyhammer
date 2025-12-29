//! Request validation for ACP session setup operations
//!
//! This module provides comprehensive validation of session setup requests
//! ensuring proper format, required parameters, and type validation with
//! detailed error reporting as required by the ACP specification.

use crate::session_errors::{SessionSetupError, SessionSetupResult};
use crate::size_validator::SizeValidator;
use agent_client_protocol::{LoadSessionRequest, NewSessionRequest, SessionId};
use serde_json::Value;

/// Comprehensive request validator for ACP session operations
pub struct RequestValidator {
    size_validator: SizeValidator,
}

impl Default for RequestValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl RequestValidator {
    /// Create a new request validator with default size limits
    pub fn new() -> Self {
        Self {
            size_validator: SizeValidator::default(),
        }
    }

    /// Validate a session/new request with comprehensive error handling
    pub fn validate_new_session_request(
        &self,
        request: &NewSessionRequest,
    ) -> SessionSetupResult<()> {
        // Validate working directory parameter (always present in ACP)
        Self::validate_working_directory_parameter(&request.cwd, "session/new")?;

        // Note: MCP server validation is deferred due to type mismatch between
        // ACP protocol types (Vec<String>) and internal config types (Vec<McpServerConfig>).
        // This will be addressed when the protocol types are aligned with internal representation.

        // Validate meta parameter format if provided
        if let Some(meta) = &request.meta {
            self.validate_meta_parameter(meta, "session/new")?;
        }

        Ok(())
    }

    /// Validate a session/load request with comprehensive error handling
    pub fn validate_load_session_request(
        &self,
        request: &LoadSessionRequest,
    ) -> SessionSetupResult<()> {
        // Validate session ID parameter (required)
        Self::validate_session_id_parameter(&request.session_id, "session/load")?;

        // Validate working directory parameter (always present in ACP)
        Self::validate_working_directory_parameter(&request.cwd, "session/load")?;

        // Note: MCP server validation is deferred due to type mismatch between
        // ACP protocol types (Vec<String>) and internal config types (Vec<McpServerConfig>).
        // This will be addressed when the protocol types are aligned with internal representation.

        // Validate meta parameter format if provided
        if let Some(meta) = &request.meta {
            self.validate_meta_parameter(meta, "session/load")?;
        }

        Ok(())
    }

    /// Validate session ID parameter format and content
    fn validate_session_id_parameter(
        session_id: &SessionId,
        request_type: &str,
    ) -> SessionSetupResult<()> {
        // Check for empty session ID
        if session_id.0.is_empty() {
            return Err(SessionSetupError::MissingRequiredParameter {
                request_type: request_type.to_string(),
                parameter_name: "sessionId".to_string(),
                parameter_type: "SessionId (ULID)".to_string(),
            });
        }

        // Validate ULID format
        crate::session_validation::validate_session_id(&session_id.0)?;

        Ok(())
    }

    /// Validate working directory parameter
    fn validate_working_directory_parameter(
        cwd: &std::path::Path,
        request_type: &str,
    ) -> SessionSetupResult<()> {
        // Check for empty path
        if cwd.as_os_str().is_empty() {
            return Err(SessionSetupError::InvalidParameterType(Box::new(
                crate::session_errors::InvalidParameterTypeDetails {
                    request_type: request_type.to_string(),
                    parameter_name: "cwd".to_string(),
                    expected_type: "non-empty PathBuf".to_string(),
                    actual_type: "empty path".to_string(),
                    provided_value: serde_json::json!(cwd.display().to_string()),
                },
            )));
        }

        // Use existing working directory validation
        crate::session_validation::validate_working_directory(cwd)?;

        Ok(())
    }

    /// Validate meta parameter format
    fn validate_meta_parameter(&self, meta: &Value, request_type: &str) -> SessionSetupResult<()> {
        // Meta should be a JSON object or null
        if !meta.is_object() && !meta.is_null() {
            return Err(SessionSetupError::InvalidParameterType(Box::new(
                crate::session_errors::InvalidParameterTypeDetails {
                    request_type: request_type.to_string(),
                    parameter_name: "meta".to_string(),
                    expected_type: "JSON object or null".to_string(),
                    actual_type: if meta.is_array() {
                        "array"
                    } else if meta.is_string() {
                        "string"
                    } else if meta.is_number() {
                        "number"
                    } else if meta.is_boolean() {
                        "boolean"
                    } else {
                        "unknown"
                    }
                    .to_string(),
                    provided_value: meta.clone(),
                },
            )));
        }

        // If it's an object, validate it doesn't contain reserved keys
        if let Some(obj) = meta.as_object() {
            let reserved_keys = ["_internal", "_system", "_acp"];
            for reserved_key in &reserved_keys {
                if obj.contains_key(*reserved_key) {
                    return Err(SessionSetupError::InvalidParameterType(Box::new(
                        crate::session_errors::InvalidParameterTypeDetails {
                            request_type: request_type.to_string(),
                            parameter_name: "meta".to_string(),
                            expected_type: "object without reserved keys".to_string(),
                            actual_type: "object with reserved key".to_string(),
                            provided_value: serde_json::json!({
                                "reservedKey": reserved_key,
                                "reservedKeys": reserved_keys
                            }),
                        },
                    )));
                }
            }

            // Check for excessively large meta objects
            let meta_str = serde_json::to_string(meta).unwrap_or_default();
            if self
                .size_validator
                .validate_meta_size(meta_str.len())
                .is_err()
            {
                let limit = self.size_validator.limits().max_meta_size;
                return Err(SessionSetupError::InvalidParameterType(Box::new(
                    crate::session_errors::InvalidParameterTypeDetails {
                        request_type: request_type.to_string(),
                        parameter_name: "meta".to_string(),
                        expected_type: format!("reasonably sized object (<{}KB)", limit / 1000),
                        actual_type: "excessively large object".to_string(),
                        provided_value: serde_json::json!({
                            "sizeBytes": meta_str.len(),
                            "maxSizeBytes": limit
                        }),
                    },
                )));
            }
        }

        Ok(())
    }

    /// Validate raw JSON request format for malformed request detection
    pub fn validate_raw_request_format(
        raw_json: &str,
        expected_method: &str,
    ) -> SessionSetupResult<Value> {
        // Parse JSON
        let parsed: Value =
            serde_json::from_str(raw_json).map_err(|e| SessionSetupError::MalformedRequest {
                request_type: expected_method.to_string(),
                details: format!("Invalid JSON format: {}", e),
                example: Some(Self::get_example_request(expected_method)),
            })?;

        // Validate JSON-RPC structure
        Self::validate_jsonrpc_structure(&parsed, expected_method)?;

        Ok(parsed)
    }

    /// Validate JSON-RPC request structure
    fn validate_jsonrpc_structure(
        request: &Value,
        expected_method: &str,
    ) -> SessionSetupResult<()> {
        let obj = request
            .as_object()
            .ok_or_else(|| SessionSetupError::MalformedRequest {
                request_type: expected_method.to_string(),
                details: "Request must be a JSON object".to_string(),
                example: Some(Self::get_example_request(expected_method)),
            })?;

        // Check required JSON-RPC fields
        if !obj.contains_key("jsonrpc") {
            return Err(SessionSetupError::MissingRequiredParameter {
                request_type: expected_method.to_string(),
                parameter_name: "jsonrpc".to_string(),
                parameter_type: "string (must be '2.0')".to_string(),
            });
        }

        if !obj.contains_key("method") {
            return Err(SessionSetupError::MissingRequiredParameter {
                request_type: expected_method.to_string(),
                parameter_name: "method".to_string(),
                parameter_type: "string".to_string(),
            });
        }

        if !obj.contains_key("id") {
            return Err(SessionSetupError::MissingRequiredParameter {
                request_type: expected_method.to_string(),
                parameter_name: "id".to_string(),
                parameter_type: "string or number".to_string(),
            });
        }

        // Validate JSON-RPC version
        if let Some(version) = obj.get("jsonrpc") {
            if version != "2.0" {
                return Err(SessionSetupError::InvalidParameterType(Box::new(
                    crate::session_errors::InvalidParameterTypeDetails {
                        request_type: expected_method.to_string(),
                        parameter_name: "jsonrpc".to_string(),
                        expected_type: "string '2.0'".to_string(),
                        actual_type: "invalid version".to_string(),
                        provided_value: version.clone(),
                    },
                )));
            }
        }

        // Validate method matches expected
        if let Some(method) = obj.get("method").and_then(|m| m.as_str()) {
            if method != expected_method {
                return Err(SessionSetupError::InvalidParameterType(Box::new(
                    crate::session_errors::InvalidParameterTypeDetails {
                        request_type: expected_method.to_string(),
                        parameter_name: "method".to_string(),
                        expected_type: format!("string '{}'", expected_method),
                        actual_type: "wrong method".to_string(),
                        provided_value: serde_json::json!(method),
                    },
                )));
            }
        }

        Ok(())
    }

    /// Get example request for error messages
    fn get_example_request(method: &str) -> String {
        match method {
            "session/new" => serde_json::to_string_pretty(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": "1",
                "method": "session/new",
                "params": {
                    "cwd": "/home/user/project",
                    "mcpServers": [],
                    "meta": {}
                }
            }))
            .unwrap_or_default(),
            "session/load" => serde_json::to_string_pretty(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": "1",
                "method": "session/load",
                "params": {
                    "sessionId": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
                    "cwd": "/home/user/project",
                    "mcpServers": [],
                    "meta": null
                }
            }))
            .unwrap_or_default(),
            _ => "See ACP documentation for proper request format".to_string(),
        }
    }

    /// Validate parameter types match expected JSON schema
    pub fn validate_parameter_types(
        params: &Value,
        expected_schema: &ParameterSchema,
        request_type: &str,
    ) -> SessionSetupResult<()> {
        let params_obj = params.as_object().ok_or_else(|| {
            SessionSetupError::InvalidParameterType(Box::new(
                crate::session_errors::InvalidParameterTypeDetails {
                    request_type: request_type.to_string(),
                    parameter_name: "params".to_string(),
                    expected_type: "object".to_string(),
                    actual_type: "not an object".to_string(),
                    provided_value: params.clone(),
                },
            ))
        })?;

        // Check required parameters
        for required_param in &expected_schema.required {
            if !params_obj.contains_key(required_param) {
                return Err(SessionSetupError::MissingRequiredParameter {
                    request_type: request_type.to_string(),
                    parameter_name: required_param.clone(),
                    parameter_type: expected_schema
                        .properties
                        .get(required_param)
                        .map(|p| p.type_name.clone())
                        .unwrap_or_else(|| "unknown".to_string()),
                });
            }
        }

        // Validate parameter types
        for (param_name, param_value) in params_obj {
            if let Some(expected_property) = expected_schema.properties.get(param_name) {
                Self::validate_single_parameter_type(
                    param_name,
                    param_value,
                    expected_property,
                    request_type,
                )?;
            }
        }

        Ok(())
    }

    /// Validate a single parameter type
    fn validate_single_parameter_type(
        param_name: &str,
        param_value: &Value,
        expected_property: &PropertySchema,
        request_type: &str,
    ) -> SessionSetupResult<()> {
        let actual_type = Self::get_json_value_type_name(param_value);

        // Check if type matches expectation
        let type_matches = match expected_property.type_name.as_str() {
            "string" => param_value.is_string(),
            "number" => param_value.is_number(),
            "boolean" => param_value.is_boolean(),
            "object" => param_value.is_object(),
            "array" => param_value.is_array(),
            "null" => param_value.is_null(),
            "PathBuf" => param_value.is_string(), // PathBuf serialized as string
            "SessionId" => param_value.is_string(), // SessionId serialized as string
            _ => true,                            // Unknown types pass through
        };

        if !type_matches && !param_value.is_null() {
            // null is often acceptable
            return Err(SessionSetupError::InvalidParameterType(Box::new(
                crate::session_errors::InvalidParameterTypeDetails {
                    request_type: request_type.to_string(),
                    parameter_name: param_name.to_string(),
                    expected_type: expected_property.type_name.clone(),
                    actual_type,
                    provided_value: param_value.clone(),
                },
            )));
        }

        // Additional validation for specific types
        match expected_property.type_name.as_str() {
            "SessionId" => {
                if let Some(session_id_str) = param_value.as_str() {
                    if crate::session_validation::validate_session_id(session_id_str).is_err() {
                        return Err(SessionSetupError::InvalidParameterType(Box::new(
                            crate::session_errors::InvalidParameterTypeDetails {
                                request_type: request_type.to_string(),
                                parameter_name: param_name.to_string(),
                                expected_type: "valid ULID format".to_string(),
                                actual_type: "invalid ULID".to_string(),
                                provided_value: param_value.clone(),
                            },
                        )));
                    }
                }
            }
            "PathBuf" => {
                if let Some(path_str) = param_value.as_str() {
                    // Check for empty path
                    if path_str.is_empty() {
                        return Err(SessionSetupError::InvalidParameterType(Box::new(
                            crate::session_errors::InvalidParameterTypeDetails {
                                request_type: request_type.to_string(),
                                parameter_name: param_name.to_string(),
                                expected_type: "non-empty path string".to_string(),
                                actual_type: "empty string".to_string(),
                                provided_value: param_value.clone(),
                            },
                        )));
                    }
                }
            }
            _ => {
                // No additional validation needed
            }
        }

        Ok(())
    }

    /// Get the type name of a JSON value for error reporting
    fn get_json_value_type_name(value: &Value) -> String {
        match value {
            Value::Null => "null".to_string(),
            Value::Bool(_) => "boolean".to_string(),
            Value::Number(_) => "number".to_string(),
            Value::String(_) => "string".to_string(),
            Value::Array(_) => "array".to_string(),
            Value::Object(_) => "object".to_string(),
        }
    }
}

/// Schema definition for parameter validation
#[derive(Debug, Clone)]
pub struct ParameterSchema {
    pub properties: std::collections::HashMap<String, PropertySchema>,
    pub required: Vec<String>,
}

/// Schema definition for individual properties
#[derive(Debug, Clone)]
pub struct PropertySchema {
    pub type_name: String,
    pub optional: bool,
}

impl ParameterSchema {
    /// Create schema for session/new request
    pub fn new_session_schema() -> Self {
        let mut properties = std::collections::HashMap::new();

        properties.insert(
            "cwd".to_string(),
            PropertySchema {
                type_name: "PathBuf".to_string(),
                optional: true,
            },
        );

        properties.insert(
            "mcpServers".to_string(),
            PropertySchema {
                type_name: "array".to_string(),
                optional: true,
            },
        );

        properties.insert(
            "meta".to_string(),
            PropertySchema {
                type_name: "object".to_string(),
                optional: true,
            },
        );

        Self {
            properties,
            required: vec![], // All parameters are optional for session/new
        }
    }

    /// Create schema for session/load request
    pub fn load_session_schema() -> Self {
        let mut properties = std::collections::HashMap::new();

        properties.insert(
            "sessionId".to_string(),
            PropertySchema {
                type_name: "SessionId".to_string(),
                optional: false,
            },
        );

        properties.insert(
            "cwd".to_string(),
            PropertySchema {
                type_name: "PathBuf".to_string(),
                optional: true,
            },
        );

        properties.insert(
            "mcpServers".to_string(),
            PropertySchema {
                type_name: "array".to_string(),
                optional: true,
            },
        );

        properties.insert(
            "meta".to_string(),
            PropertySchema {
                type_name: "object".to_string(),
                optional: true,
            },
        );

        Self {
            properties,
            required: vec!["sessionId".to_string()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_new_session_request() -> NewSessionRequest {
        NewSessionRequest {
            cwd: std::env::current_dir().unwrap(),
            mcp_servers: vec![],
            meta: Some(serde_json::json!({"test": true})),
        }
    }

    fn create_test_load_session_request() -> LoadSessionRequest {
        LoadSessionRequest {
            session_id: SessionId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string()),
            cwd: std::env::current_dir().unwrap(),
            mcp_servers: vec![],
            meta: None,
        }
    }

    #[test]
    fn test_validate_new_session_request_valid() {
        let validator = RequestValidator::new();
        let request = create_test_new_session_request();
        let result = validator.validate_new_session_request(&request);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_load_session_request_valid() {
        let validator = RequestValidator::new();
        let request = create_test_load_session_request();
        let result = validator.validate_load_session_request(&request);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_load_session_request_invalid_session_id() {
        let validator = RequestValidator::new();
        let mut request = create_test_load_session_request();
        request.session_id = SessionId::new("invalid-session-id".to_string());

        let result = validator.validate_load_session_request(&request);
        assert!(result.is_err());

        if let Err(SessionSetupError::InvalidSessionId { .. }) = result {
            // Expected error type
        } else {
            panic!("Expected InvalidSessionId error");
        }
    }

    #[test]
    fn test_validate_working_directory_parameter_empty() {
        let empty_path = PathBuf::from("");
        let result = RequestValidator::validate_working_directory_parameter(&empty_path, "test");

        assert!(result.is_err());
        if let Err(SessionSetupError::InvalidParameterType(..)) = result {
            // Expected error type
        } else {
            panic!("Expected InvalidParameterType error");
        }
    }

    #[test]
    fn test_validate_meta_parameter_invalid_type() {
        let validator = RequestValidator::new();
        let invalid_meta = serde_json::json!("not an object");
        let result = validator.validate_meta_parameter(&invalid_meta, "test");

        assert!(result.is_err());
        if let Err(SessionSetupError::InvalidParameterType(..)) = result {
            // Expected error type
        } else {
            panic!("Expected InvalidParameterType error");
        }
    }

    #[test]
    fn test_validate_meta_parameter_reserved_key() {
        let validator = RequestValidator::new();
        let meta_with_reserved = serde_json::json!({"_system": "reserved"});
        let result = validator.validate_meta_parameter(&meta_with_reserved, "test");

        assert!(result.is_err());
        if let Err(SessionSetupError::InvalidParameterType(..)) = result {
            // Expected error type
        } else {
            panic!("Expected InvalidParameterType error");
        }
    }

    #[test]
    fn test_validate_raw_request_format_valid() {
        let valid_json = r#"{
            "jsonrpc": "2.0",
            "id": "1",
            "method": "session/new",
            "params": {}
        }"#;

        let result = RequestValidator::validate_raw_request_format(valid_json, "session/new");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_raw_request_format_invalid_json() {
        let invalid_json = r#"{ invalid json }"#;
        let result = RequestValidator::validate_raw_request_format(invalid_json, "session/new");

        assert!(result.is_err());
        if let Err(SessionSetupError::MalformedRequest { .. }) = result {
            // Expected error type
        } else {
            panic!("Expected MalformedRequest error");
        }
    }

    #[test]
    fn test_validate_jsonrpc_structure_missing_field() {
        let request_without_method = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "1"
        });

        let result =
            RequestValidator::validate_jsonrpc_structure(&request_without_method, "session/new");
        assert!(result.is_err());

        if let Err(SessionSetupError::MissingRequiredParameter { parameter_name, .. }) = result {
            assert_eq!(parameter_name, "method");
        } else {
            panic!("Expected MissingRequiredParameter error for method");
        }
    }

    #[test]
    fn test_validate_jsonrpc_structure_wrong_version() {
        let request_wrong_version = serde_json::json!({
            "jsonrpc": "1.0",
            "id": "1",
            "method": "session/new"
        });

        let result =
            RequestValidator::validate_jsonrpc_structure(&request_wrong_version, "session/new");
        assert!(result.is_err());

        if let Err(SessionSetupError::InvalidParameterType(details)) = result {
            assert_eq!(details.parameter_name, "jsonrpc");
        } else {
            panic!("Expected InvalidParameterType error for jsonrpc version");
        }
    }

    #[test]
    fn test_parameter_schema_creation() {
        let new_session_schema = ParameterSchema::new_session_schema();
        assert!(new_session_schema.properties.contains_key("cwd"));
        assert!(new_session_schema.properties.contains_key("mcpServers"));
        assert!(new_session_schema.properties.contains_key("meta"));
        assert!(new_session_schema.required.is_empty());

        let load_session_schema = ParameterSchema::load_session_schema();
        assert!(load_session_schema.properties.contains_key("sessionId"));
        assert_eq!(load_session_schema.required, vec!["sessionId".to_string()]);
    }

    #[test]
    fn test_get_json_value_type_name() {
        assert_eq!(
            RequestValidator::get_json_value_type_name(&serde_json::json!(null)),
            "null"
        );
        assert_eq!(
            RequestValidator::get_json_value_type_name(&serde_json::json!(true)),
            "boolean"
        );
        assert_eq!(
            RequestValidator::get_json_value_type_name(&serde_json::json!(123)),
            "number"
        );
        assert_eq!(
            RequestValidator::get_json_value_type_name(&serde_json::json!("test")),
            "string"
        );
        assert_eq!(
            RequestValidator::get_json_value_type_name(&serde_json::json!([])),
            "array"
        );
        assert_eq!(
            RequestValidator::get_json_value_type_name(&serde_json::json!({})),
            "object"
        );
    }

    #[test]
    fn test_get_example_request() {
        let new_example = RequestValidator::get_example_request("session/new");
        assert!(new_example.contains("session/new"));
        assert!(new_example.contains("jsonrpc"));

        let load_example = RequestValidator::get_example_request("session/load");
        assert!(load_example.contains("session/load"));
        assert!(load_example.contains("sessionId"));
    }
}
