//! Capability validation for ACP session setup operations
//!
//! This module provides comprehensive validation of agent and client capabilities
//! ensuring ACP compliance and proper error reporting for capability mismatches.

use crate::session_errors::{SessionSetupError, SessionSetupResult};
use agent_client_protocol::{AgentCapabilities, ClientCapabilities};
use serde_json::Value;
use std::collections::HashSet;

/// Comprehensive capability validator for ACP compliance
pub struct CapabilityValidator {
    /// Known capability names for validation
    known_agent_capabilities: HashSet<String>,
    known_client_capabilities: HashSet<String>,
}

impl Default for CapabilityValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl CapabilityValidator {
    /// Create a new capability validator with default known capabilities
    pub fn new() -> Self {
        let mut known_agent_capabilities = HashSet::new();
        known_agent_capabilities.insert("load_session".to_string());
        known_agent_capabilities.insert("mcp".to_string());
        known_agent_capabilities.insert("stdio".to_string());
        known_agent_capabilities.insert("http".to_string());
        known_agent_capabilities.insert("sse".to_string());

        // Known client capabilities include both top-level fields and meta capabilities:
        // - "terminal": top-level boolean field in ClientCapabilities
        // - "notifications", "progress": meta capabilities (validated as booleans)
        // - "cancellation": reserved for future use in meta
        let mut known_client_capabilities = HashSet::new();
        known_client_capabilities.insert("notifications".to_string());
        known_client_capabilities.insert("progress".to_string());
        known_client_capabilities.insert("cancellation".to_string());
        known_client_capabilities.insert("terminal".to_string());

        Self {
            known_agent_capabilities,
            known_client_capabilities,
        }
    }

    /// Validate agent capabilities for session setup operations
    pub fn validate_agent_capabilities(
        &self,
        capabilities: &AgentCapabilities,
        requested_operations: &[String],
    ) -> SessionSetupResult<()> {
        // Validate loadSession capability if session loading is requested
        if requested_operations.contains(&"session/load".to_string()) && !capabilities.load_session
        {
            return Err(SessionSetupError::LoadSessionNotSupported {
                declared_capability: false,
            });
        }

        // Validate MCP transport capabilities if MCP servers are requested
        let has_mcp_request = requested_operations
            .iter()
            .any(|op| op.contains("mcp") || op.contains("server"));

        if has_mcp_request {
            self.validate_mcp_transport_capabilities(capabilities, requested_operations)?;
        }

        Ok(())
    }

    /// Validate MCP transport capabilities
    fn validate_mcp_transport_capabilities(
        &self,
        capabilities: &AgentCapabilities,
        requested_operations: &[String],
    ) -> SessionSetupResult<()> {
        // Check if any transport-specific operations are requested
        for operation in requested_operations {
            if operation.contains("http") && !capabilities.mcp_capabilities.http {
                return Err(SessionSetupError::TransportNotSupported {
                    requested_transport: "http".to_string(),
                    declared_capability: false,
                    supported_transports: self.get_supported_transports(capabilities),
                });
            }

            if operation.contains("sse") && !capabilities.mcp_capabilities.sse {
                return Err(SessionSetupError::TransportNotSupported {
                    requested_transport: "sse".to_string(),
                    declared_capability: false,
                    supported_transports: self.get_supported_transports(capabilities),
                });
            }
        }

        Ok(())
    }

    /// Get list of supported transport types from capabilities
    fn get_supported_transports(&self, capabilities: &AgentCapabilities) -> Vec<String> {
        let mut supported = Vec::new();

        // STDIO is always supported (it's the base transport)
        supported.push("stdio".to_string());

        if capabilities.mcp_capabilities.http {
            supported.push("http".to_string());
        }

        if capabilities.mcp_capabilities.sse {
            supported.push("sse".to_string());
        }

        supported
    }

    /// Validate client capabilities for compatibility
    /// Validate client capabilities for session setup compatibility
    ///
    /// Performs validation in three stages:
    /// 1. Meta capabilities (optional) - structural and type validation
    /// 2. Filesystem capabilities - structural validation of meta field
    /// 3. Terminal capability - boolean field, validated separately via validate_terminal_capability if needed
    ///
    /// All validations use a lenient approach: unknown capabilities are logged but don't fail validation.
    /// This supports forward compatibility when clients declare newer capabilities.
    pub fn validate_client_capabilities(
        &self,
        capabilities: Option<&ClientCapabilities>,
    ) -> SessionSetupResult<()> {
        if let Some(client_caps) = capabilities {
            // Validate meta capabilities if present
            if let Some(meta) = &client_caps.meta {
                self.validate_client_meta_capabilities(meta)?;
            }

            // Validate filesystem capabilities structure
            self.validate_client_filesystem_capabilities(&client_caps.fs)?;

            // Terminal capability is a boolean, no additional validation needed
            // Specific terminal requirements can be checked with validate_terminal_capability
        }

        Ok(())
    }

    /// Validate client meta capabilities format and values
    ///
    /// # Arguments
    /// * `meta` - The meta capabilities JSON object to validate
    ///
    /// # Returns
    /// * `Ok(())` if validation passes
    /// * `Err(SessionSetupError::CapabilityFormatError)` if validation fails
    ///
    /// # Errors
    /// Returns `CapabilityFormatError` if:
    /// - Meta is not a JSON object
    /// - Known meta capabilities (`streaming`, `notifications`, `progress`) have wrong types (must be boolean)
    ///
    /// Unknown meta capabilities are logged but don't fail validation (lenient approach for forward compatibility).
    fn validate_client_meta_capabilities(&self, meta: &Value) -> SessionSetupResult<()> {
        if !meta.is_object() {
            return Err(SessionSetupError::CapabilityFormatError {
                capability_name: "meta".to_string(),
                expected_format: "object".to_string(),
                actual_value: meta.clone(),
            });
        }

        // Validate that meta values have appropriate types
        if let Some(meta_obj) = meta.as_object() {
            for (key, value) in meta_obj {
                // Common meta capabilities should be booleans
                match key.as_str() {
                    "streaming" | "notifications" | "progress" => {
                        if !value.is_boolean() {
                            return Err(SessionSetupError::CapabilityFormatError {
                                capability_name: format!("meta.{}", key),
                                expected_format: "boolean".to_string(),
                                actual_value: value.clone(),
                            });
                        }
                    }
                    _ => {
                        // Unknown meta capabilities are logged as warnings but don't fail validation
                        tracing::debug!("Unknown client meta capability: {}", key);
                    }
                }
            }
        }

        Ok(())
    }

    /// Validate client filesystem capabilities structure
    ///
    /// # Arguments
    /// * `fs_caps` - The filesystem capabilities to validate
    ///
    /// # Returns
    /// * `Ok(())` if validation passes
    /// * `Err(SessionSetupError::CapabilityFormatError)` if validation fails
    ///
    /// # Errors
    /// Returns `CapabilityFormatError` if:
    /// - fs.meta field (if present) is not a JSON object
    ///
    /// The boolean fields (`read_text_file`, `write_text_file`) are always valid.
    /// Unknown fs.meta capabilities are logged but don't fail validation.
    fn validate_client_filesystem_capabilities(
        &self,
        fs_caps: &agent_client_protocol::FileSystemCapability,
    ) -> SessionSetupResult<()> {
        // Validate meta field if present
        if let Some(fs_meta) = &fs_caps.meta {
            if !fs_meta.is_object() {
                return Err(SessionSetupError::CapabilityFormatError {
                    capability_name: "fs.meta".to_string(),
                    expected_format: "object".to_string(),
                    actual_value: fs_meta.clone(),
                });
            }
        }

        // read_text_file and write_text_file are boolean fields that are always valid
        // No additional validation needed for these primitive boolean fields
        Ok(())
    }

    /// Validate capability format from JSON value
    pub fn validate_capability_format(
        &self,
        capability_name: &str,
        capability_value: &Value,
        expected_type: &str,
    ) -> SessionSetupResult<()> {
        match expected_type {
            "boolean" => {
                if !capability_value.is_boolean() {
                    return Err(SessionSetupError::CapabilityFormatError {
                        capability_name: capability_name.to_string(),
                        expected_format: "boolean".to_string(),
                        actual_value: capability_value.clone(),
                    });
                }
            }
            "object" => {
                if !capability_value.is_object() {
                    return Err(SessionSetupError::CapabilityFormatError {
                        capability_name: capability_name.to_string(),
                        expected_format: "object".to_string(),
                        actual_value: capability_value.clone(),
                    });
                }
            }
            "array" => {
                if !capability_value.is_array() {
                    return Err(SessionSetupError::CapabilityFormatError {
                        capability_name: capability_name.to_string(),
                        expected_format: "array".to_string(),
                        actual_value: capability_value.clone(),
                    });
                }
            }
            "string" => {
                if !capability_value.is_string() {
                    return Err(SessionSetupError::CapabilityFormatError {
                        capability_name: capability_name.to_string(),
                        expected_format: "string".to_string(),
                        actual_value: capability_value.clone(),
                    });
                }
            }
            _ => {
                // Unknown expected type - this is a validation error in our own code
                return Err(SessionSetupError::CapabilityFormatError {
                    capability_name: capability_name.to_string(),
                    expected_format: expected_type.to_string(),
                    actual_value: capability_value.clone(),
                });
            }
        }

        Ok(())
    }

    /// Validate that capability names are known/supported
    pub fn validate_capability_names(
        &self,
        agent_capabilities: Option<&Value>,
        client_capabilities: Option<&Value>,
    ) -> SessionSetupResult<()> {
        // Validate agent capability names
        if let Some(agent_caps) = agent_capabilities {
            if let Some(agent_obj) = agent_caps.as_object() {
                for capability_name in agent_obj.keys() {
                    if !self.known_agent_capabilities.contains(capability_name) {
                        return Err(SessionSetupError::UnknownCapability {
                            capability_name: capability_name.clone(),
                            known_capabilities: self
                                .known_agent_capabilities
                                .iter()
                                .cloned()
                                .collect(),
                        });
                    }
                }
            }
        }

        // Validate client capability names
        if let Some(client_caps) = client_capabilities {
            if let Some(client_obj) = client_caps.as_object() {
                for capability_name in client_obj.keys() {
                    if !self.known_client_capabilities.contains(capability_name) {
                        // For client capabilities, we're more lenient - just log unknown ones
                        tracing::warn!("Unknown client capability: {}", capability_name);
                    }
                }
            }
        }

        Ok(())
    }

    /// Check capability compatibility between agent and client
    pub fn check_capability_compatibility(
        &self,
        agent_capabilities: &AgentCapabilities,
        client_capabilities: Option<&ClientCapabilities>,
    ) -> SessionSetupResult<Vec<String>> {
        let mut compatibility_warnings = Vec::new();

        // Check if client expects features that agent doesn't support
        if let Some(_client_caps) = client_capabilities {
            // Example compatibility checks:
            // - If client expects progress notifications but agent doesn't support them
            // - If client expects cancellation but agent doesn't support it
            // For now, we don't have specific compatibility requirements
        }

        // Check if agent capabilities have requirements that client might not meet
        if agent_capabilities.load_session {
            // Client should support session notifications for proper session loading
            compatibility_warnings.push(
                "Agent supports session loading - ensure client handles session/update notifications".to_string()
            );
        }

        if agent_capabilities.mcp_capabilities.http || agent_capabilities.mcp_capabilities.sse {
            compatibility_warnings.push(
                "Agent supports HTTP/SSE MCP transports - ensure client can handle network-based MCP servers".to_string()
            );
        }

        Ok(compatibility_warnings)
    }

    /// Validate transport capability requirements for specific MCP server configs
    pub fn validate_transport_requirements(
        &self,
        agent_capabilities: &AgentCapabilities,
        mcp_servers: &[crate::config::McpServerConfig],
    ) -> SessionSetupResult<()> {
        for server_config in mcp_servers {
            match server_config {
                crate::config::McpServerConfig::Stdio(_) => {
                    // STDIO is always supported - no validation needed
                }
                crate::config::McpServerConfig::Http(_) => {
                    if !agent_capabilities.mcp_capabilities.http {
                        return Err(SessionSetupError::TransportNotSupported {
                            requested_transport: "http".to_string(),
                            declared_capability: false,
                            supported_transports: self.get_supported_transports(agent_capabilities),
                        });
                    }
                }
                crate::config::McpServerConfig::Sse(_) => {
                    if !agent_capabilities.mcp_capabilities.sse {
                        return Err(SessionSetupError::TransportNotSupported {
                            requested_transport: "sse".to_string(),
                            declared_capability: false,
                            supported_transports: self.get_supported_transports(agent_capabilities),
                        });
                    }
                }
            }
        }

        Ok(())
    }

    /// Add custom capability name to known capabilities
    pub fn add_known_agent_capability(&mut self, capability_name: String) {
        self.known_agent_capabilities.insert(capability_name);
    }

    /// Add custom client capability name to known capabilities
    pub fn add_known_client_capability(&mut self, capability_name: String) {
        self.known_client_capabilities.insert(capability_name);
    }

    /// Validate terminal capability for ACP compliance
    pub fn validate_terminal_capability(
        &self,
        client_capabilities: Option<&ClientCapabilities>,
    ) -> SessionSetupResult<()> {
        match client_capabilities {
            Some(caps) => {
                if !caps.terminal {
                    return Err(SessionSetupError::CapabilityNotSupported {
                        capability_name: "terminal".to_string(),
                        required_for: "terminal capability is required for terminal operations".to_string(),
                    });
                }
                Ok(())
            }
            None => Err(SessionSetupError::CapabilityNotSupported {
                capability_name: "terminal".to_string(),
                required_for: "terminal capability is required for terminal operations - no client capabilities provided".to_string(),
            }),
        }
    }

    /// Check if terminal operations should be available based on client capabilities
    pub fn is_terminal_supported(client_capabilities: Option<&ClientCapabilities>) -> bool {
        client_capabilities
            .map(|caps| caps.terminal)
            .unwrap_or(false)
    }
}

/// Capability requirement checker for specific operations
pub struct CapabilityRequirementChecker;

impl CapabilityRequirementChecker {
    /// Check if all requirements are met for session/new operation
    pub fn check_new_session_requirements(
        agent_capabilities: &AgentCapabilities,
        mcp_servers: &[crate::config::McpServerConfig],
    ) -> SessionSetupResult<()> {
        let validator = CapabilityValidator::new();

        // Check transport requirements
        validator.validate_transport_requirements(agent_capabilities, mcp_servers)?;

        Ok(())
    }

    /// Check if all requirements are met for session/load operation
    pub fn check_load_session_requirements(
        agent_capabilities: &AgentCapabilities,
        mcp_servers: &[crate::config::McpServerConfig],
    ) -> SessionSetupResult<()> {
        let validator = CapabilityValidator::new();

        // Check loadSession capability
        if !agent_capabilities.load_session {
            return Err(SessionSetupError::LoadSessionNotSupported {
                declared_capability: false,
            });
        }

        // Check transport requirements
        validator.validate_transport_requirements(agent_capabilities, mcp_servers)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Test imports are above in the test function

    fn create_test_agent_capabilities() -> AgentCapabilities {
        AgentCapabilities {
            load_session: true,
            prompt_capabilities: agent_client_protocol::PromptCapabilities {
                image: false,
                audio: false,
                embedded_context: false,
                meta: None,
            },
            mcp_capabilities: agent_client_protocol::McpCapabilities {
                http: true,
                sse: false,
                meta: None,
            },
            meta: None,
        }
    }

    #[test]
    fn test_capability_validator_creation() {
        let validator = CapabilityValidator::new();
        assert!(validator.known_agent_capabilities.contains("load_session"));
        assert!(validator.known_agent_capabilities.contains("mcp"));
        assert!(validator
            .known_client_capabilities
            .contains("notifications"));
    }

    #[test]
    fn test_validate_agent_capabilities_load_session_supported() {
        let validator = CapabilityValidator::new();
        let capabilities = create_test_agent_capabilities();
        let operations = vec!["session/load".to_string()];

        let result = validator.validate_agent_capabilities(&capabilities, &operations);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_agent_capabilities_load_session_not_supported() {
        let validator = CapabilityValidator::new();
        let mut capabilities = create_test_agent_capabilities();
        capabilities.load_session = false;
        let operations = vec!["session/load".to_string()];

        let result = validator.validate_agent_capabilities(&capabilities, &operations);
        assert!(result.is_err());

        if let Err(SessionSetupError::LoadSessionNotSupported { .. }) = result {
            // Expected error type
        } else {
            panic!("Expected LoadSessionNotSupported error");
        }
    }

    #[test]
    fn test_validate_transport_capabilities_http_supported() {
        let validator = CapabilityValidator::new();
        let capabilities = create_test_agent_capabilities();
        let operations = vec!["mcp_http".to_string()];

        let result = validator.validate_agent_capabilities(&capabilities, &operations);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_transport_capabilities_sse_not_supported() {
        let validator = CapabilityValidator::new();
        let capabilities = create_test_agent_capabilities();
        let operations = vec!["mcp_sse".to_string()];

        let result = validator.validate_agent_capabilities(&capabilities, &operations);
        assert!(result.is_err());

        if let Err(SessionSetupError::TransportNotSupported { .. }) = result {
            // Expected error type
        } else {
            panic!("Expected TransportNotSupported error");
        }
    }

    #[test]
    fn test_get_supported_transports() {
        let validator = CapabilityValidator::new();
        let capabilities = create_test_agent_capabilities();

        let supported = validator.get_supported_transports(&capabilities);
        assert!(supported.contains(&"stdio".to_string()));
        assert!(supported.contains(&"http".to_string()));
        assert!(!supported.contains(&"sse".to_string()));
    }

    #[test]
    fn test_validate_capability_format_boolean() {
        let validator = CapabilityValidator::new();
        let valid_bool = serde_json::json!(true);
        let invalid_bool = serde_json::json!("not_a_boolean");

        assert!(validator
            .validate_capability_format("test", &valid_bool, "boolean")
            .is_ok());
        assert!(validator
            .validate_capability_format("test", &invalid_bool, "boolean")
            .is_err());
    }

    #[test]
    fn test_validate_capability_format_object() {
        let validator = CapabilityValidator::new();
        let valid_object = serde_json::json!({"key": "value"});
        let invalid_object = serde_json::json!("not_an_object");

        assert!(validator
            .validate_capability_format("test", &valid_object, "object")
            .is_ok());
        assert!(validator
            .validate_capability_format("test", &invalid_object, "object")
            .is_err());
    }

    #[test]
    fn test_validate_unknown_capability() {
        let validator = CapabilityValidator::new();
        let unknown_agent_caps = serde_json::json!({
            "unknown_capability": true
        });

        let result = validator.validate_capability_names(Some(&unknown_agent_caps), None);
        assert!(result.is_err());

        if let Err(SessionSetupError::UnknownCapability { .. }) = result {
            // Expected error type
        } else {
            panic!("Expected UnknownCapability error");
        }
    }

    #[test]
    fn test_check_capability_compatibility() {
        let validator = CapabilityValidator::new();
        let capabilities = create_test_agent_capabilities();

        let warnings = validator
            .check_capability_compatibility(&capabilities, None)
            .unwrap();
        assert!(!warnings.is_empty()); // Should have at least one warning about session loading
    }

    #[test]
    fn test_capability_requirement_checker_new_session() {
        let capabilities = create_test_agent_capabilities();
        let mcp_servers = vec![];

        let result = CapabilityRequirementChecker::check_new_session_requirements(
            &capabilities,
            &mcp_servers,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_capability_requirement_checker_load_session_supported() {
        let capabilities = create_test_agent_capabilities();
        let mcp_servers = vec![];

        let result = CapabilityRequirementChecker::check_load_session_requirements(
            &capabilities,
            &mcp_servers,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_capability_requirement_checker_load_session_not_supported() {
        let mut capabilities = create_test_agent_capabilities();
        capabilities.load_session = false;
        let mcp_servers = vec![];

        let result = CapabilityRequirementChecker::check_load_session_requirements(
            &capabilities,
            &mcp_servers,
        );
        assert!(result.is_err());

        if let Err(SessionSetupError::LoadSessionNotSupported { .. }) = result {
            // Expected error type
        } else {
            panic!("Expected LoadSessionNotSupported error");
        }
    }

    #[test]
    fn test_add_custom_capabilities() {
        let mut validator = CapabilityValidator::new();
        validator.add_known_agent_capability("custom_agent_cap".to_string());
        validator.add_known_client_capability("custom_client_cap".to_string());

        assert!(validator
            .known_agent_capabilities
            .contains("custom_agent_cap"));
        assert!(validator
            .known_client_capabilities
            .contains("custom_client_cap"));
    }

    fn create_test_client_capabilities_with_terminal(terminal_enabled: bool) -> ClientCapabilities {
        use agent_client_protocol::{ClientCapabilities, FileSystemCapability};

        ClientCapabilities {
            fs: FileSystemCapability {
                read_text_file: true,
                write_text_file: true,
                meta: None,
            },
            terminal: terminal_enabled,
            meta: None,
        }
    }

    #[test]
    fn test_validate_terminal_capability_supported() {
        let validator = CapabilityValidator::new();
        let capabilities = create_test_client_capabilities_with_terminal(true);

        let result = validator.validate_terminal_capability(Some(&capabilities));
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_terminal_capability_not_supported() {
        let validator = CapabilityValidator::new();
        let capabilities = create_test_client_capabilities_with_terminal(false);

        let result = validator.validate_terminal_capability(Some(&capabilities));
        assert!(result.is_err());

        match result {
            Err(SessionSetupError::CapabilityNotSupported {
                capability_name,
                required_for,
            }) => {
                assert_eq!(capability_name, "terminal");
                assert_eq!(
                    required_for,
                    "terminal capability is required for terminal operations"
                );
            }
            _ => panic!("Expected CapabilityNotSupported error"),
        }
    }

    #[test]
    fn test_validate_terminal_capability_no_capabilities() {
        let validator = CapabilityValidator::new();

        let result = validator.validate_terminal_capability(None);
        assert!(result.is_err());

        match result {
            Err(SessionSetupError::CapabilityNotSupported {
                capability_name,
                required_for,
            }) => {
                assert_eq!(capability_name, "terminal");
                assert!(required_for.contains("no client capabilities provided"));
            }
            _ => panic!("Expected CapabilityNotSupported error"),
        }
    }

    #[test]
    fn test_is_terminal_supported() {
        // Test with terminal enabled
        let capabilities_enabled = create_test_client_capabilities_with_terminal(true);
        assert!(CapabilityValidator::is_terminal_supported(Some(
            &capabilities_enabled
        )));

        // Test with terminal disabled
        let capabilities_disabled = create_test_client_capabilities_with_terminal(false);
        assert!(!CapabilityValidator::is_terminal_supported(Some(
            &capabilities_disabled
        )));

        // Test with no capabilities
        assert!(!CapabilityValidator::is_terminal_supported(None));
    }

    #[test]
    fn test_terminal_capability_known_capabilities() {
        let validator = CapabilityValidator::new();

        // Terminal should be in known client capabilities
        assert!(validator.known_client_capabilities.contains("terminal"));

        // Test that terminal capability name validation passes
        let client_caps_with_terminal = serde_json::json!({
            "terminal": true,
            "notifications": true
        });

        let result = validator.validate_capability_names(None, Some(&client_caps_with_terminal));
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_client_capabilities_with_minimal_required_fields() {
        let validator = CapabilityValidator::new();
        let capabilities = create_test_client_capabilities_with_terminal(true);

        let result = validator.validate_client_capabilities(Some(&capabilities));
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_client_capabilities_none() {
        let validator = CapabilityValidator::new();

        let result = validator.validate_client_capabilities(None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_client_capabilities_with_valid_meta() {
        use agent_client_protocol::{ClientCapabilities, FileSystemCapability};

        let validator = CapabilityValidator::new();
        let capabilities = ClientCapabilities {
            fs: FileSystemCapability {
                read_text_file: true,
                write_text_file: false,
                meta: None,
            },
            terminal: true,
            meta: Some(serde_json::json!({
                "streaming": true,
                "notifications": false,
                "progress": true
            })),
        };

        let result = validator.validate_client_capabilities(Some(&capabilities));
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_client_capabilities_with_invalid_meta_type() {
        use agent_client_protocol::{ClientCapabilities, FileSystemCapability};

        let validator = CapabilityValidator::new();
        let capabilities = ClientCapabilities {
            fs: FileSystemCapability {
                read_text_file: true,
                write_text_file: true,
                meta: None,
            },
            terminal: true,
            meta: Some(serde_json::json!("not_an_object")),
        };

        let result = validator.validate_client_capabilities(Some(&capabilities));
        assert!(result.is_err());

        match result {
            Err(SessionSetupError::CapabilityFormatError {
                capability_name,
                expected_format,
                ..
            }) => {
                assert_eq!(capability_name, "meta");
                assert_eq!(expected_format, "object");
            }
            _ => panic!("Expected CapabilityFormatError for invalid meta type"),
        }
    }

    #[test]
    fn test_validate_client_capabilities_with_invalid_meta_value() {
        use agent_client_protocol::{ClientCapabilities, FileSystemCapability};

        let validator = CapabilityValidator::new();
        let capabilities = ClientCapabilities {
            fs: FileSystemCapability {
                read_text_file: true,
                write_text_file: true,
                meta: None,
            },
            terminal: false,
            meta: Some(serde_json::json!({
                "streaming": "not_a_boolean"
            })),
        };

        let result = validator.validate_client_capabilities(Some(&capabilities));
        assert!(result.is_err());

        match result {
            Err(SessionSetupError::CapabilityFormatError {
                capability_name,
                expected_format,
                ..
            }) => {
                assert_eq!(capability_name, "meta.streaming");
                assert_eq!(expected_format, "boolean");
            }
            _ => panic!("Expected CapabilityFormatError for invalid meta value type"),
        }
    }

    #[test]
    fn test_validate_client_capabilities_with_unknown_meta_capability() {
        use agent_client_protocol::{ClientCapabilities, FileSystemCapability};

        let validator = CapabilityValidator::new();
        let capabilities = ClientCapabilities {
            fs: FileSystemCapability {
                read_text_file: true,
                write_text_file: true,
                meta: None,
            },
            terminal: true,
            meta: Some(serde_json::json!({
                "unknown_capability": true
            })),
        };

        // Unknown meta capabilities should not fail validation, just log debug
        let result = validator.validate_client_capabilities(Some(&capabilities));
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_client_capabilities_with_fs_meta() {
        use agent_client_protocol::{ClientCapabilities, FileSystemCapability};

        let validator = CapabilityValidator::new();
        let capabilities = ClientCapabilities {
            fs: FileSystemCapability {
                read_text_file: true,
                write_text_file: true,
                meta: Some(serde_json::json!({
                    "encoding": "utf-8"
                })),
            },
            terminal: true,
            meta: None,
        };

        let result = validator.validate_client_capabilities(Some(&capabilities));
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_client_capabilities_with_invalid_fs_meta() {
        use agent_client_protocol::{ClientCapabilities, FileSystemCapability};

        let validator = CapabilityValidator::new();
        let capabilities = ClientCapabilities {
            fs: FileSystemCapability {
                read_text_file: true,
                write_text_file: false,
                meta: Some(serde_json::json!("not_an_object")),
            },
            terminal: false,
            meta: None,
        };

        let result = validator.validate_client_capabilities(Some(&capabilities));
        assert!(result.is_err());

        match result {
            Err(SessionSetupError::CapabilityFormatError {
                capability_name,
                expected_format,
                ..
            }) => {
                assert_eq!(capability_name, "fs.meta");
                assert_eq!(expected_format, "object");
            }
            _ => panic!("Expected CapabilityFormatError for invalid fs.meta type"),
        }
    }

    #[test]
    fn test_validate_client_capabilities_with_all_optional_fields_populated() {
        use agent_client_protocol::{ClientCapabilities, FileSystemCapability};

        let validator = CapabilityValidator::new();
        let capabilities = ClientCapabilities {
            fs: FileSystemCapability {
                read_text_file: true,
                write_text_file: false,
                meta: Some(serde_json::json!({
                    "encoding": "utf-8",
                    "permissions": true
                })),
            },
            terminal: true,
            meta: Some(serde_json::json!({
                "streaming": false,
                "notifications": true,
                "progress": false
            })),
        };

        let result = validator.validate_client_capabilities(Some(&capabilities));
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_client_capabilities_empty_meta() {
        use agent_client_protocol::{ClientCapabilities, FileSystemCapability};

        let validator = CapabilityValidator::new();

        let capabilities = ClientCapabilities {
            fs: FileSystemCapability {
                read_text_file: true,
                write_text_file: true,
                meta: None,
            },
            terminal: true,
            meta: Some(serde_json::json!({})),
        };

        let result = validator.validate_client_capabilities(Some(&capabilities));
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_client_capabilities_empty_fs_meta() {
        use agent_client_protocol::{ClientCapabilities, FileSystemCapability};

        let validator = CapabilityValidator::new();

        let capabilities = ClientCapabilities {
            fs: FileSystemCapability {
                read_text_file: true,
                write_text_file: true,
                meta: Some(serde_json::json!({})),
            },
            terminal: true,
            meta: None,
        };

        let result = validator.validate_client_capabilities(Some(&capabilities));
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_client_capabilities_mixed_valid_invalid_meta() {
        use agent_client_protocol::{ClientCapabilities, FileSystemCapability};

        let validator = CapabilityValidator::new();

        let capabilities = ClientCapabilities {
            fs: FileSystemCapability {
                read_text_file: true,
                write_text_file: true,
                meta: None,
            },
            terminal: true,
            meta: Some(serde_json::json!({
                "streaming": true,
                "notifications": "invalid_string_value"
            })),
        };

        let result = validator.validate_client_capabilities(Some(&capabilities));
        assert!(result.is_err());

        match result {
            Err(SessionSetupError::CapabilityFormatError {
                capability_name,
                expected_format,
                ..
            }) => {
                assert_eq!(capability_name, "meta.notifications");
                assert_eq!(expected_format, "boolean");
            }
            _ => panic!("Expected CapabilityFormatError"),
        }
    }
}
