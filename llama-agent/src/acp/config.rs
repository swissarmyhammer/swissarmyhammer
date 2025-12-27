use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::error::ConfigError;
use super::permissions::PermissionPolicy;

/// Default maximum file size for read operations (10 MB)
const DEFAULT_MAX_FILE_SIZE: u64 = 10_485_760;

/// Default terminal output buffer size (1 MB)
const DEFAULT_TERMINAL_OUTPUT_BUFFER_BYTES: usize = 1_048_576;

/// Default graceful shutdown timeout in seconds
const DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT_SECS: u64 = 5;

/// Configuration for the Anthropic Claude Protocol (ACP) integration.
///
/// This structure defines the protocol version, agent capabilities, permission policies,
/// and filesystem access controls for ACP client interactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpConfig {
    /// Protocol version to advertise
    pub protocol_version: String,

    /// Capabilities to advertise
    pub capabilities: AcpCapabilities,

    /// Permission policy configuration
    pub permission_policy: PermissionPolicy,

    /// File system access restrictions
    pub filesystem: FilesystemSettings,

    /// Terminal settings
    pub terminal: TerminalSettings,

    /// Available session modes
    ///
    /// Defines the modes that can be used with this agent.
    /// If empty and supports_modes is true, no modes will be returned.
    #[serde(skip, default)]
    pub available_modes: Vec<agent_client_protocol::SessionMode>,

    /// Default mode ID
    ///
    /// The mode to use when creating new sessions. Must be in available_modes list.
    #[serde(default = "default_mode_id_value")]
    pub default_mode_id: String,

    /// Default MCP servers to include in all sessions
    ///
    /// These servers will be automatically added to every new session,
    /// in addition to any servers specified in the NewSessionRequest.
    #[serde(skip, default)]
    pub default_mcp_servers: Vec<agent_client_protocol::McpServer>,
}

fn default_mode_id_value() -> String {
    "general-purpose".to_string()
}

impl Default for AcpConfig {
    fn default() -> Self {
        Self {
            protocol_version: "0.1.0".to_string(),
            capabilities: AcpCapabilities::default(),
            permission_policy: PermissionPolicy::AlwaysAsk,
            filesystem: FilesystemSettings::default(),
            terminal: TerminalSettings::default(),
            available_modes: Vec::new(),
            default_mode_id: "general-purpose".to_string(),
            default_mcp_servers: Vec::new(),
        }
    }
}

impl AcpConfig {
    /// Load configuration from a YAML file
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the YAML configuration file
    ///
    /// # Returns
    ///
    /// Returns the loaded configuration or an error if the file cannot be read or parsed.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use llama_agent::acp::AcpConfig;
    ///
    /// let config = AcpConfig::from_file("acp-config.yaml")?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).map_err(|e| {
            ConfigError::FileReadError(format!(
                "Failed to read config file {}: {}",
                path.display(),
                e
            ))
        })?;

        serde_yaml::from_str(&content)
            .map_err(|e| ConfigError::ParseError(format!("Failed to parse YAML config: {}", e)))
    }

    /// Save configuration to a YAML file
    ///
    /// # Arguments
    ///
    /// * `path` - Path where the YAML configuration file should be written
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the file was written successfully, or an error otherwise.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use llama_agent::acp::AcpConfig;
    ///
    /// let config = AcpConfig::default();
    /// config.to_file("acp-config.yaml")?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), ConfigError> {
        let path = path.as_ref();
        let content = serde_yaml::to_string(self).map_err(|e| {
            ConfigError::SerializationError(format!("Failed to serialize config to YAML: {}", e))
        })?;

        std::fs::write(path, content).map_err(|e| {
            ConfigError::FileWriteError(format!(
                "Failed to write config file {}: {}",
                path.display(),
                e
            ))
        })
    }
}

/// Capabilities advertised to ACP clients.
///
/// This structure indicates which features and operations this agent supports,
/// including session management, modes, plans, slash commands, filesystem operations,
/// and terminal access.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpCapabilities {
    pub supports_session_loading: bool,
    pub supports_modes: bool,
    pub supports_plans: bool,
    pub supports_slash_commands: bool,
    pub filesystem: FilesystemCapabilities,
    pub terminal: bool,
}

impl Default for AcpCapabilities {
    fn default() -> Self {
        Self {
            supports_session_loading: true,
            supports_modes: true,
            supports_plans: true,
            supports_slash_commands: true,
            filesystem: FilesystemCapabilities::default(),
            terminal: true,
        }
    }
}

/// Filesystem operation capabilities that can be advertised to clients.
///
/// Indicates which filesystem operations (read, write) are supported by this agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilesystemCapabilities {
    pub read_text_file: bool,
    pub write_text_file: bool,
}

impl Default for FilesystemCapabilities {
    fn default() -> Self {
        Self {
            read_text_file: true,
            write_text_file: true,
        }
    }
}

/// Filesystem access control settings.
///
/// Defines path restrictions and file size limits for filesystem operations,
/// providing security boundaries for ACP client interactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilesystemSettings {
    /// Allowed paths (absolute paths or patterns)
    pub allowed_paths: Vec<PathBuf>,

    /// Blocked paths
    pub blocked_paths: Vec<PathBuf>,

    /// Maximum file size for read operations (bytes)
    pub max_file_size: u64,
}

impl Default for FilesystemSettings {
    fn default() -> Self {
        Self {
            allowed_paths: vec![],
            blocked_paths: vec![],
            max_file_size: DEFAULT_MAX_FILE_SIZE,
        }
    }
}

/// Newtype wrapper for graceful shutdown timeout duration
///
/// Provides type safety to prevent mixing up timeout durations with other Duration values.
/// This ensures that timeout configurations cannot be accidentally confused with other
/// time-based parameters in the system.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GracefulShutdownTimeout(#[serde(with = "duration_secs")] Duration);

impl GracefulShutdownTimeout {
    /// Create a new graceful shutdown timeout
    pub fn new(duration: Duration) -> Self {
        Self(duration)
    }

    /// Get the timeout as a Duration
    pub fn as_duration(&self) -> Duration {
        self.0
    }
}

impl Default for GracefulShutdownTimeout {
    fn default() -> Self {
        Self(Duration::from_secs(DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT_SECS))
    }
}

/// Serialization helper for Duration as seconds
mod duration_secs {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_secs())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(secs))
    }
}

/// Terminal settings for process management.
///
/// Defines resource limits and timeout configuration for terminal operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSettings {
    /// Maximum buffer size for terminal output (bytes)
    pub output_buffer_bytes: usize,

    /// Graceful shutdown timeout before escalating to SIGKILL
    ///
    /// When killing a terminal process, the system first sends SIGTERM to allow
    /// graceful shutdown. If the process doesn't exit within this timeout,
    /// SIGKILL is sent to forcibly terminate it.
    pub graceful_shutdown_timeout: GracefulShutdownTimeout,
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self {
            output_buffer_bytes: DEFAULT_TERMINAL_OUTPUT_BUFFER_BYTES,
            graceful_shutdown_timeout: GracefulShutdownTimeout::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acp_config_default() {
        let config = AcpConfig::default();
        assert_eq!(config.protocol_version, "0.1.0");
        assert!(config.capabilities.supports_session_loading);
        assert!(config.capabilities.supports_modes);
        assert!(config.capabilities.supports_plans);
        assert!(config.capabilities.supports_slash_commands);
        assert!(config.capabilities.terminal);
    }

    #[test]
    fn test_acp_capabilities_default() {
        let caps = AcpCapabilities::default();
        assert!(caps.supports_session_loading);
        assert!(caps.supports_modes);
        assert!(caps.supports_plans);
        assert!(caps.supports_slash_commands);
        assert!(caps.terminal);
        assert!(caps.filesystem.read_text_file);
        assert!(caps.filesystem.write_text_file);
    }

    #[test]
    fn test_filesystem_capabilities_default() {
        let caps = FilesystemCapabilities::default();
        assert!(caps.read_text_file);
        assert!(caps.write_text_file);
    }

    #[test]
    fn test_filesystem_settings_default() {
        let settings = FilesystemSettings::default();
        assert_eq!(settings.max_file_size, DEFAULT_MAX_FILE_SIZE);
        assert!(settings.allowed_paths.is_empty());
        assert!(settings.blocked_paths.is_empty());
    }

    #[test]
    fn test_acp_capabilities_serialization() {
        let caps = AcpCapabilities::default();
        let json = serde_json::to_string(&caps).unwrap();

        // Verify camelCase field names in JSON output
        assert!(json.contains("supportsSessionLoading"));
        assert!(json.contains("supportsModes"));
        assert!(json.contains("supportsPlans"));
        assert!(json.contains("supportsSlashCommands"));
        assert!(json.contains("readTextFile"));
        assert!(json.contains("writeTextFile"));
    }

    #[test]
    fn test_filesystem_settings_serialization() {
        let settings = FilesystemSettings::default();
        let json = serde_json::to_string(&settings).unwrap();

        // Verify camelCase field names in JSON output
        assert!(json.contains("allowedPaths"));
        assert!(json.contains("blockedPaths"));
        assert!(json.contains("maxFileSize"));
    }

    #[test]
    fn test_terminal_settings_default() {
        let settings = TerminalSettings::default();
        assert_eq!(
            settings.output_buffer_bytes,
            DEFAULT_TERMINAL_OUTPUT_BUFFER_BYTES
        );
    }

    #[test]
    fn test_acp_config_with_terminal_settings() {
        let config = AcpConfig::default();
        assert_eq!(
            config.terminal.output_buffer_bytes,
            DEFAULT_TERMINAL_OUTPUT_BUFFER_BYTES
        );
        assert_eq!(
            config.terminal.graceful_shutdown_timeout.as_duration(),
            std::time::Duration::from_secs(DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT_SECS)
        );
    }

    #[test]
    fn test_graceful_shutdown_timeout_default() {
        let timeout = GracefulShutdownTimeout::default();
        assert_eq!(
            timeout.as_duration(),
            std::time::Duration::from_secs(DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT_SECS)
        );
    }

    #[test]
    fn test_graceful_shutdown_timeout_custom() {
        let custom_duration = std::time::Duration::from_secs(10);
        let timeout = GracefulShutdownTimeout::new(custom_duration);
        assert_eq!(timeout.as_duration(), custom_duration);
    }

    #[test]
    fn test_terminal_settings_serialization() {
        let settings = TerminalSettings::default();
        let json = serde_json::to_string(&settings).unwrap();

        // Verify camelCase field names in JSON output
        assert!(json.contains("outputBufferBytes"));
        assert!(json.contains("gracefulShutdownTimeout"));
    }

    #[test]
    fn test_graceful_shutdown_timeout_serialization() {
        let timeout = GracefulShutdownTimeout::new(std::time::Duration::from_secs(10));
        let json = serde_json::to_value(timeout).unwrap();

        // Should serialize as number of seconds
        assert_eq!(json, serde_json::json!(10));
    }

    #[test]
    fn test_graceful_shutdown_timeout_deserialization() {
        let json = serde_json::json!(10);
        let timeout: GracefulShutdownTimeout = serde_json::from_value(json).unwrap();
        assert_eq!(timeout.as_duration(), std::time::Duration::from_secs(10));
    }

    #[test]
    fn test_acp_config_from_file() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let yaml_config = r#"
protocolVersion: "0.2.0"
capabilities:
  supportsSessionLoading: true
  supportsModes: true
  supportsPlans: true
  supportsSlashCommands: true
  filesystem:
    readTextFile: true
    writeTextFile: true
  terminal: true
permissionPolicy: autoApproveReads
filesystem:
  allowedPaths:
    - /home/user/projects
  blockedPaths:
    - /home/user/.ssh
  maxFileSize: 5242880
terminal:
  outputBufferBytes: 524288
  gracefulShutdownTimeout: 10
defaultModeId: "general-purpose"
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(yaml_config.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let config = AcpConfig::from_file(temp_file.path()).unwrap();

        assert_eq!(config.protocol_version, "0.2.0");
        assert_eq!(config.filesystem.max_file_size, 5242880);
        assert_eq!(config.filesystem.allowed_paths.len(), 1);
        assert_eq!(config.filesystem.blocked_paths.len(), 1);
        assert_eq!(config.terminal.output_buffer_bytes, 524288);
        assert_eq!(
            config.terminal.graceful_shutdown_timeout.as_duration(),
            std::time::Duration::from_secs(10)
        );
    }

    #[test]
    fn test_acp_config_from_file_not_found() {
        let result = AcpConfig::from_file("/nonexistent/path/config.yaml");
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::acp::error::ConfigError::FileReadError(_) => {}
            _ => panic!("Expected FileReadError"),
        }
    }

    #[test]
    fn test_acp_config_from_file_invalid_yaml() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let invalid_yaml = r#"
protocolVersion: "0.2.0"
capabilities:
  supportsSessionLoading: true
  - this is invalid yaml structure
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(invalid_yaml.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let result = AcpConfig::from_file(temp_file.path());
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::acp::error::ConfigError::ParseError(_) => {}
            _ => panic!("Expected ParseError"),
        }
    }

    #[test]
    fn test_acp_config_to_file() {
        use tempfile::NamedTempFile;

        let config = AcpConfig::default();
        let temp_file = NamedTempFile::new().unwrap();

        config.to_file(temp_file.path()).unwrap();

        // Verify file was written and can be read back
        let loaded_config = AcpConfig::from_file(temp_file.path()).unwrap();
        assert_eq!(loaded_config.protocol_version, config.protocol_version);
        assert_eq!(
            loaded_config.filesystem.max_file_size,
            config.filesystem.max_file_size
        );
    }

    #[test]
    fn test_acp_config_round_trip() {
        use tempfile::NamedTempFile;

        let config = AcpConfig {
            protocol_version: "1.2.3".to_string(),
            filesystem: FilesystemSettings {
                max_file_size: 9999999,
                allowed_paths: vec![
                    std::path::PathBuf::from("/path/one"),
                    std::path::PathBuf::from("/path/two"),
                ],
                ..Default::default()
            },
            terminal: TerminalSettings {
                output_buffer_bytes: 2048576,
                ..Default::default()
            },
            ..Default::default()
        };

        let temp_file = NamedTempFile::new().unwrap();
        config.to_file(temp_file.path()).unwrap();

        let loaded_config = AcpConfig::from_file(temp_file.path()).unwrap();
        assert_eq!(loaded_config.protocol_version, "1.2.3");
        assert_eq!(loaded_config.filesystem.max_file_size, 9999999);
        assert_eq!(loaded_config.filesystem.allowed_paths.len(), 2);
        assert_eq!(loaded_config.terminal.output_buffer_bytes, 2048576);
    }
}
