#![cfg(feature = "acp")]

use llama_agent::acp::config::{FilesystemSettings, TerminalSettings};
use llama_agent::acp::AcpConfig;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_acp_config_from_file_basic() {
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
permissionPolicy: alwaysAsk
filesystem:
  allowedPaths:
    - /home/user/projects
  blockedPaths:
    - /home/user/.ssh
  maxFileSize: 5242880
terminal:
  outputBufferBytes: 524288
  gracefulShutdownTimeout: 10
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

    let err = result.unwrap_err();
    let err_string = format!("{}", err);
    assert!(err_string.contains("Failed to read config file"));
}

#[test]
fn test_acp_config_from_file_invalid_yaml() {
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

    let err = result.unwrap_err();
    let err_string = format!("{}", err);
    assert!(err_string.contains("Failed to parse YAML config"));
}

#[test]
fn test_acp_config_to_file() {
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

#[test]
fn test_acp_config_to_file_creates_valid_yaml() {
    let config = AcpConfig::default();
    let temp_file = NamedTempFile::new().unwrap();

    config.to_file(temp_file.path()).unwrap();

    // Read the file content and verify it's valid YAML
    let content = std::fs::read_to_string(temp_file.path()).unwrap();

    // Should contain camelCase fields
    assert!(content.contains("protocolVersion"));
    assert!(content.contains("permissionPolicy"));
    assert!(content.contains("supportsSessionLoading"));
    assert!(content.contains("maxFileSize"));
    assert!(content.contains("outputBufferBytes"));

    // Verify it can be parsed back
    let _parsed: AcpConfig = serde_yaml::from_str(&content).unwrap();
}
