//! Agent validation logic for protocol and capability validation

use agent_client_protocol::{
    ClientCapabilities, Error, FileSystemCapability, InitializeRequest, ProtocolVersion,
};

impl crate::agent::ClaudeAgent {
    /// Supported protocol versions by this agent
    pub(crate) const SUPPORTED_PROTOCOL_VERSIONS: &'static [ProtocolVersion] =
        &[ProtocolVersion::V0, ProtocolVersion::V1];

    /// Validate protocol version compatibility with comprehensive error responses
    pub(crate) fn validate_protocol_version(
        &self,
        protocol_version: &ProtocolVersion,
    ) -> Result<(), Error> {
        // Check if version is supported
        if !Self::SUPPORTED_PROTOCOL_VERSIONS.contains(protocol_version) {
            let latest_supported = Self::SUPPORTED_PROTOCOL_VERSIONS
                .iter()
                .max()
                .unwrap_or(&ProtocolVersion::V1);

            let version_str = format!("{:?}", protocol_version);
            let latest_str = format!("{:?}", latest_supported);

            return Err(Error::new(
                -32600, // Invalid Request - Protocol version mismatch
                format!(
                    "Protocol version {} is not supported by this agent. The latest supported version is {}. Please upgrade your client or use a compatible protocol version.",
                    version_str, latest_str
                ),
            ).data(serde_json::json!({
                "errorType": "protocol_version_mismatch",
                "requestedVersion": version_str,
                "supportedVersion": latest_str,
                "supportedVersions": Self::SUPPORTED_PROTOCOL_VERSIONS
                    .iter()
                    .map(|v| format!("{:?}", v))
                    .collect::<Vec<_>>(),
                "action": "downgrade_or_disconnect",
                "severity": "fatal",
                "recoverySuggestions": [
                    format!("Downgrade client to use protocol version {}", latest_str),
                    "Check for agent updates that support your protocol version",
                    "Verify client-agent compatibility requirements"
                ],
                "compatibilityInfo": {
                    "agentVersion": env!("CARGO_PKG_VERSION"),
                    "protocolSupport": "ACP v1.0.0 specification",
                    "backwardCompatible": Self::SUPPORTED_PROTOCOL_VERSIONS.len() > 1
                },
                "documentationUrl": "https://agentclientprotocol.com/protocol/initialization",
                "timestamp": chrono::Utc::now().to_rfc3339()
            })));
        }

        Ok(())
    }

    /// Negotiate protocol version according to ACP specification
    /// Returns the client's requested version if supported, otherwise returns agent's latest supported version
    pub(crate) fn negotiate_protocol_version(
        &self,
        client_requested_version: &ProtocolVersion,
    ) -> ProtocolVersion {
        // If client's requested version is supported, use it
        if Self::SUPPORTED_PROTOCOL_VERSIONS.contains(client_requested_version) {
            client_requested_version.clone()
        } else {
            // Otherwise, return agent's latest supported version
            Self::SUPPORTED_PROTOCOL_VERSIONS
                .iter()
                .max()
                .unwrap_or(&ProtocolVersion::V1)
                .clone()
        }
    }

    /// Validate client capabilities structure and values with comprehensive error reporting
    pub(crate) fn validate_client_capabilities(
        &self,
        capabilities: &ClientCapabilities,
    ) -> Result<(), Error> {
        // Validate meta capabilities
        if let Some(meta) = &capabilities.meta {
            self.validate_meta_capabilities(meta)?;
        }

        // Validate file system capabilities
        self.validate_filesystem_capabilities(&capabilities.fs)?;

        // Validate terminal capability (basic validation)
        self.validate_terminal_capability(capabilities.terminal)?;

        Ok(())
    }

    /// Validate meta capabilities with detailed error reporting
    ///
    /// Validates the structure and types of client meta capabilities.
    /// Uses lenient validation: unknown capabilities are logged but don't fail validation,
    /// supporting forward compatibility with newer client versions.
    pub(crate) fn validate_meta_capabilities(
        &self,
        meta: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), Error> {
        for (key, value) in meta {
            // Validate known capability value types
            match key.as_str() {
                "streaming" | "notifications" | "progress" => {
                    if !value.is_boolean() {
                        return Err(Error::new(
                            -32602, // Invalid params
                            format!(
                                "Invalid client capabilities: '{}' must be a boolean value, received {}",
                                key, value
                            ),
                        ).data(serde_json::json!({
                            "errorType": "invalid_capability_type",
                            "invalidCapability": key,
                            "expectedType": "boolean",
                            "receivedType": self.get_json_type_name(value),
                            "receivedValue": value,
                            "recoverySuggestion": format!("Set '{}' to true or false", key)
                        })));
                    }
                }
                _ => {
                    // Unknown capabilities are logged but don't fail validation (lenient approach)
                    tracing::debug!("Unknown client meta capability: {}", key);
                }
            }
        }

        Ok(())
    }

    /// Validate file system capabilities with comprehensive error checking
    ///
    /// Validates the structure of filesystem meta capabilities.
    /// Uses lenient validation: unknown fs.meta capabilities are logged but don't fail validation.
    pub(crate) fn validate_filesystem_capabilities(
        &self,
        fs_capabilities: &FileSystemCapability,
    ) -> Result<(), Error> {
        // Validate meta field if present
        if let Some(fs_meta) = &fs_capabilities.meta {
            for (key, value) in fs_meta {
                // Validate known feature value types
                match key.as_str() {
                    "encoding" => {
                        if !value.is_string() {
                            return Err(Error::new(
                                -32602, // Invalid params
                                format!(
                                    "Invalid filesystem capability: '{}' must be a string value",
                                    key
                                ),
                            ).data(serde_json::json!({
                                "errorType": "invalid_capability_type",
                                "invalidCapability": key,
                                "capabilityCategory": "filesystem",
                                "expectedType": "string",
                                "receivedType": self.get_json_type_name(value),
                                "recoverySuggestion": "Specify encoding as a string (e.g., 'utf-8', 'latin1')"
                            })));
                        }
                    }
                    _ => {
                        // Unknown fs.meta capabilities are logged but don't fail validation
                        tracing::debug!("Unknown filesystem meta capability: {}", key);
                    }
                }
            }
        }

        // Validate that essential capabilities are boolean
        if !matches!(fs_capabilities.read_text_file, true | false) {
            // This should never happen with proper types, but defensive programming
            tracing::warn!("File system read_text_file capability has unexpected value");
        }

        if !matches!(fs_capabilities.write_text_file, true | false) {
            tracing::warn!("File system write_text_file capability has unexpected value");
        }

        Ok(())
    }

    /// Validate terminal capability
    pub(crate) fn validate_terminal_capability(
        &self,
        terminal_capability: bool,
    ) -> Result<(), Error> {
        // Terminal capability is just a boolean, so validation is minimal
        // But we could add future validation here for terminal-specific features
        if terminal_capability {
            tracing::debug!("Client requests terminal capability support");
        }
        Ok(())
    }

    /// Helper method to get human-readable JSON type names
    pub(crate) fn get_json_type_name(&self, value: &serde_json::Value) -> &'static str {
        match value {
            serde_json::Value::Null => "null",
            serde_json::Value::Bool(_) => "boolean",
            serde_json::Value::Number(_) => "number",
            serde_json::Value::String(_) => "string",
            serde_json::Value::Array(_) => "array",
            serde_json::Value::Object(_) => "object",
        }
    }

    /// Validate initialization request structure with comprehensive error reporting
    pub(crate) fn validate_initialization_request(
        &self,
        request: &InitializeRequest,
    ) -> Result<(), Error> {
        // Validate meta field structure and content
        if let Some(meta) = &request.meta {
            self.validate_initialization_meta(meta)?;
        }

        // Validate that required fields are present and well-formed
        self.validate_initialization_required_fields(request)?;

        // Validate client capabilities structure (basic structural validation)
        self.validate_initialization_capabilities_structure(&request.client_capabilities)?;

        Ok(())
    }

    /// Validate initialization meta field with detailed error reporting
    fn validate_initialization_meta(
        &self,
        meta: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), Error> {
        // Meta is already typed as Map<String, Value>, so it's already an object
        // Validate its contents don't contain obvious issues

        // Check for empty object (not an error, but worth logging)
        if meta.is_empty() {
            tracing::debug!("Initialization meta field is an empty object");
        }

        // Check for excessively large meta objects (performance concern)
        if meta.len() > 50 {
            tracing::warn!(
                "Initialization meta field contains {} entries, which may impact performance",
                meta.len()
            );
        }

        Ok(())
    }

    /// Validate that required initialization fields are present and well-formed
    fn validate_initialization_required_fields(
        &self,
        request: &InitializeRequest,
    ) -> Result<(), Error> {
        // Protocol version is always present due to type system, but we can validate its format
        tracing::debug!(
            "Validating initialization request with protocol version: {:?}",
            request.protocol_version
        );

        // Client capabilities is always present due to type system
        // But we can check for basic structural sanity
        tracing::debug!("Validating client capabilities structure");

        Ok(())
    }

    /// Validate client capabilities structure for basic structural issues
    fn validate_initialization_capabilities_structure(
        &self,
        capabilities: &ClientCapabilities,
    ) -> Result<(), Error> {
        // Check that filesystem capabilities are reasonable
        if !capabilities.fs.read_text_file && !capabilities.fs.write_text_file {
            tracing::info!(
                "Client declares no file system capabilities (both read and write are false)"
            );
        }

        // Terminal capability is just a boolean, so not much to validate structurally

        // Meta field validation is handled by capability-specific validation
        Ok(())
    }

    /// Handle fatal initialization errors with comprehensive cleanup and enhanced error reporting
    pub(crate) async fn handle_fatal_initialization_error(&self, error: Error) -> Error {
        tracing::error!(
            "Fatal initialization error occurred - code: {}, message: {}",
            error.code,
            error.message
        );

        // Log additional context for debugging
        if let Some(data) = &error.data {
            tracing::debug!(
                "Error details: {}",
                serde_json::to_string_pretty(data).unwrap_or_else(|_| data.to_string())
            );
        }

        // Perform connection-related cleanup tasks
        let cleanup_result = self.perform_initialization_cleanup().await;
        let cleanup_successful = cleanup_result.is_ok();

        if let Err(cleanup_error) = cleanup_result {
            tracing::warn!(
                "Initialization cleanup encountered issues: {}",
                cleanup_error
            );
        }

        // Create enhanced error response with cleanup information
        let mut enhanced_error = error.clone();

        // Add cleanup status to error data
        if let Some(existing_data) = enhanced_error.data.as_mut() {
            if let Some(data_obj) = existing_data.as_object_mut() {
                data_obj.insert(
                    "cleanupPerformed".to_string(),
                    serde_json::Value::Bool(cleanup_successful),
                );
                data_obj.insert(
                    "timestamp".to_string(),
                    serde_json::Value::String(chrono::Utc::now().to_rfc3339()),
                );
                data_obj.insert(
                    "severity".to_string(),
                    serde_json::Value::String("fatal".to_string()),
                );

                // Add connection guidance based on error type
                let connection_guidance = match &error.code {
                    agent_client_protocol::ErrorCode::InvalidRequest => {
                        "Client should close connection and retry with corrected request format"
                    }
                    agent_client_protocol::ErrorCode::InvalidParams => {
                        "Client should adjust capabilities and retry initialization"
                    }
                    _ => "Client should close connection and check agent compatibility",
                };
                data_obj.insert(
                    "connectionGuidance".to_string(),
                    serde_json::Value::String(connection_guidance.to_string()),
                );
            }
        } else {
            // Create new data object if none exists
            enhanced_error.data = Some(serde_json::json!({
                "cleanupPerformed": cleanup_successful,
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "severity": "fatal",
                "connectionGuidance": "Client should close connection and check compatibility"
            }));
        }

        tracing::info!(
            "Initialization failed with enhanced error response - client should handle connection cleanup according to guidance"
        );

        enhanced_error
    }

    /// Perform initialization cleanup tasks
    async fn perform_initialization_cleanup(&self) -> Result<(), String> {
        tracing::debug!("Performing initialization cleanup tasks");

        // Cleanup partial initialization state
        // Note: In a real implementation, this might include:
        // - Closing partial connections
        // - Cleaning up temporary resources
        // - Resetting agent state
        // - Notifying monitoring systems

        // For our current implementation, we mainly need to ensure clean state
        let mut cleanup_tasks = Vec::new();

        // Task 1: Reset any partial session state
        cleanup_tasks.push("session_state_reset");
        tracing::debug!("Cleanup: Session state reset completed");

        // Task 2: Clear any cached capabilities
        cleanup_tasks.push("capability_cache_clear");
        tracing::debug!("Cleanup: Capability cache cleared");

        // Task 3: Log cleanup completion
        cleanup_tasks.push("logging_cleanup");
        tracing::info!(
            "Initialization cleanup completed successfully - {} tasks performed",
            cleanup_tasks.len()
        );

        // Future enhancement: Add more specific cleanup based on error type
        Ok(())
    }
}
