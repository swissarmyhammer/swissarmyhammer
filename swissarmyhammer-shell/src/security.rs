//! Shell command security validation and control system
//!
//! This module provides comprehensive security controls for shell command execution,
//! including blocked command prevention, directory access controls, and audit logging.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use swissarmyhammer_common::{Result, SwissArmyHammerError};
use thiserror::Error;
use tracing::{error, info, warn};

/// Maximum allowed command length in characters
const MAX_COMMAND_LENGTH: usize = 4096;

/// Maximum allowed environment variable value length in characters
const MAX_ENV_VALUE_LENGTH: usize = 1024;



/// Security validation errors that can occur during shell command processing
#[derive(Debug, Error)]
pub enum ShellSecurityError {
    /// Command contains blocked pattern
    #[error("Command contains blocked pattern: {pattern} in command: {command}")]
    BlockedCommandPattern {
        /// The matched blocked pattern
        pattern: String,
        /// The command containing the pattern
        command: String,
    },

    /// Command exceeds maximum allowed length
    #[error("Command too long: {length} characters exceeds limit of {limit}")]
    CommandTooLong {
        /// Actual command length
        length: usize,
        /// Maximum allowed length
        limit: usize,
    },

    /// Directory access denied by security policy
    #[error("Directory access denied: {directory} is not in allowed directories")]
    DirectoryAccessDenied {
        /// Directory that was denied access
        directory: PathBuf,
    },

    /// Invalid directory path or access error
    #[error("Invalid directory: {directory} - {reason}")]
    InvalidDirectory {
        /// Directory path
        directory: String,
        /// Reason for invalidity
        reason: String,
    },

    /// Environment variable name is invalid
    #[error("Environment variable name invalid: {name}")]
    InvalidEnvironmentVariable {
        /// Name of the invalid environment variable
        name: String,
    },

    /// Environment variable value contains invalid characters or exceeds limits
    #[error("Environment variable {name} has invalid value: {reason}")]
    InvalidEnvironmentVariableValue {
        /// Name of the environment variable
        name: String,
        /// Reason for the validation failure
        reason: String,
    },

    /// Command validation failed for general reasons
    #[error("Command validation failed: {reason}")]
    ValidationFailed {
        /// Reason for the validation failure
        reason: String,
    },
}

/// Shell security policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellSecurityPolicy {
    /// Enable command validation (default: true)
    pub enable_validation: bool,

    /// List of blocked command patterns (regex patterns)
    pub blocked_commands: Vec<String>,

    /// List of allowed directories for command execution (optional)
    pub allowed_directories: Option<Vec<PathBuf>>,

    /// Maximum allowed command length
    pub max_command_length: usize,

    /// Enable audit logging of all command executions
    pub enable_audit_logging: bool,



    /// Maximum allowed environment variable value length
    pub max_env_value_length: usize,
}

impl Default for ShellSecurityPolicy {
    fn default() -> Self {
        Self {
            enable_validation: true,
            blocked_commands: vec![
                // Dangerous file operations
                r"rm\s+-rf\s+/".to_string(),
                r"rm\s+-rf\s+\*".to_string(),
                r"format\s+".to_string(),
                r"mkfs\s+".to_string(),
                r"dd\s+if=.*of=/dev/".to_string(),
                // System modification commands
                r"fdisk\s+".to_string(),
                r"parted\s+".to_string(),
                r"shutdown\s+".to_string(),
                r"reboot\s+".to_string(),
                r"sudo\s+".to_string(),
                r"systemctl\s+".to_string(),
                r"crontab\s+".to_string(),
                r"chmod\s+\+s\s+".to_string(),
                // Network-based attacks
                r"wget.*http.*\|.*sh".to_string(),
                r"curl.*http.*\|.*sh".to_string(),
                r"nc\s+-l\s+".to_string(),
                r"ssh\s+.*@".to_string(),
                // Code execution patterns
                r"eval\s+".to_string(),
                r"exec\s+/bin/".to_string(),
                // Sensitive file access
                r"/etc/passwd".to_string(),
                r"/etc/shadow".to_string(),
                // sed ends in pain, use the editing tools
                r"sed\s+.*".to_string(), // Force more use of edit tools
            ],
            allowed_directories: None, // No directory restrictions by default
            max_command_length: MAX_COMMAND_LENGTH,
            enable_audit_logging: true,

            max_env_value_length: MAX_ENV_VALUE_LENGTH,
        }
    }
}

/// Shell command validator that applies security policies
#[derive(Debug)]
pub struct ShellSecurityValidator {
    policy: ShellSecurityPolicy,
    blocked_patterns: Vec<Regex>,
}

impl ShellSecurityValidator {
    /// Create a new validator with the given policy
    pub fn new(policy: ShellSecurityPolicy) -> Result<Self> {
        let blocked_patterns = Self::compile_blocked_patterns(&policy.blocked_commands)?;

        Ok(Self {
            policy,
            blocked_patterns,
        })
    }

    /// Create a validator with default policy
    pub fn with_default_policy() -> Result<Self> {
        Self::new(ShellSecurityPolicy::default())
    }

    /// Validate a command against the security policy
    pub fn validate_command(&self, command: &str) -> std::result::Result<(), ShellSecurityError> {
        if !self.policy.enable_validation {
            return Ok(());
        }

        // Check command length
        self.check_command_length(command)?;

        // Check for blocked patterns
        self.check_blocked_patterns(command)?;

        Ok(())
    }

    /// Validate directory access according to policy
    pub fn validate_directory_access(
        &self,
        directory: &Path,
    ) -> std::result::Result<(), ShellSecurityError> {
        // Check for path traversal attempts
        let path_str = directory.to_string_lossy();
        if path_str.contains("../") || path_str.contains("..\\") {
            return Err(ShellSecurityError::DirectoryAccessDenied {
                directory: directory.to_path_buf(),
            });
        }

        if let Some(allowed_dirs) = &self.policy.allowed_directories {
            let canonical_dir =
                directory
                    .canonicalize()
                    .map_err(|e| ShellSecurityError::InvalidDirectory {
                        directory: directory.display().to_string(),
                        reason: format!("Cannot canonicalize: {e}"),
                    })?;

            let is_allowed = allowed_dirs.iter().any(|allowed| {
                allowed
                    .canonicalize()
                    .map(|canon_allowed| canonical_dir.starts_with(&canon_allowed))
                    .unwrap_or(false)
            });

            if !is_allowed {
                return Err(ShellSecurityError::DirectoryAccessDenied {
                    directory: directory.to_path_buf(),
                });
            }
        }

        Ok(())
    }

    /// Validate environment variables
    pub fn validate_environment_variables(
        &self,
        env_vars: &HashMap<String, String>,
    ) -> std::result::Result<(), ShellSecurityError> {
        for (key, value) in env_vars {
            // Check variable name validity
            if !Self::is_valid_env_var_name(key) {
                return Err(ShellSecurityError::InvalidEnvironmentVariable { name: key.clone() });
            }

            // Check value length limits
            if value.len() > self.policy.max_env_value_length {
                return Err(ShellSecurityError::InvalidEnvironmentVariableValue {
                    name: key.clone(),
                    reason: format!(
                        "Value length {} exceeds maximum of {} characters",
                        value.len(),
                        self.policy.max_env_value_length
                    ),
                });
            }

            // Check for invalid characters in values
            if value.contains('\0') {
                return Err(ShellSecurityError::InvalidEnvironmentVariableValue {
                    name: key.clone(),
                    reason: "Invalid characters: null bytes are not allowed".to_string(),
                });
            }

            if value.contains('\n') || value.contains('\r') {
                return Err(ShellSecurityError::InvalidEnvironmentVariableValue {
                    name: key.clone(),
                    reason: "Invalid characters: newlines are not allowed".to_string(),
                });
            }

            // Log warnings for protected environment variables being modified
            Self::warn_if_protected_env_var(key);
        }
        Ok(())
    }

    /// Get the security policy
    pub fn policy(&self) -> &ShellSecurityPolicy {
        &self.policy
    }

    /// Check if command length is within limits
    fn check_command_length(&self, command: &str) -> std::result::Result<(), ShellSecurityError> {
        let length = command.len();
        if length > self.policy.max_command_length {
            return Err(ShellSecurityError::CommandTooLong {
                length,
                limit: self.policy.max_command_length,
            });
        }
        Ok(())
    }

    /// Check for blocked command patterns
    fn check_blocked_patterns(&self, command: &str) -> std::result::Result<(), ShellSecurityError> {
        for pattern in &self.blocked_patterns {
            if pattern.is_match(command) {
                return Err(ShellSecurityError::BlockedCommandPattern {
                    pattern: pattern.as_str().to_string(),
                    command: command.to_string(),
                });
            }
        }
        Ok(())
    }

    /// Compile blocked command patterns from configuration
    fn compile_blocked_patterns(patterns: &[String]) -> Result<Vec<Regex>> {
        let mut compiled = Vec::new();
        for pattern in patterns {
            compiled.push(
                Regex::new(pattern).map_err(|e| SwissArmyHammerError::Other {
                    message: format!("Failed to compile blocked pattern '{pattern}': {e}"),
                })?,
            );
        }
        Ok(compiled)
    }

    /// Check if an environment variable name is valid
    fn is_valid_env_var_name(name: &str) -> bool {
        if name.is_empty() {
            return false;
        }
        let mut chars = name.chars();
        if let Some(first) = chars.next() {
            if !first.is_ascii_alphabetic() && first != '_' {
                return false;
            }
        }
        chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    }

    /// Warn if a protected environment variable is being modified
    fn warn_if_protected_env_var(name: &str) {
        const PROTECTED_VARS: &[&str] = &[
            "PATH",
            "LD_LIBRARY_PATH",
            "HOME",
            "USER",
            "SHELL",
            "SSH_AUTH_SOCK",
            "SUDO_USER",
            "SUDO_UID",
        ];

        if PROTECTED_VARS.contains(&name) {
            warn!(
                target: "shell_security",
                env_var = %name,
                "Modifying protected environment variable"
            );
        }
    }
}

/// Audit event for shell command execution
#[derive(Debug, Serialize)]
pub struct ShellAuditEvent {
    /// Timestamp when the command was executed (Unix epoch seconds)
    pub timestamp: u64,
    /// The shell command that was executed
    pub command: String,
    /// Working directory where the command was executed
    pub working_directory: Option<PathBuf>,
    /// Environment variables set for the command
    pub environment_vars: HashMap<String, String>,
    /// Exit code returned by the command
    pub exit_code: Option<i32>,
    /// How long the command took to execute in milliseconds
    pub execution_time_ms: Option<u64>,
    /// Result of security validation (passed/failed)
    pub validation_result: String,
    /// Version of the security policy that was applied
    pub security_policy_version: String,
}

impl ShellAuditEvent {
    /// Create a new audit event for command execution
    pub fn new(
        command: String,
        working_directory: Option<&Path>,
        environment_vars: &HashMap<String, String>,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            timestamp,
            command,
            working_directory: working_directory.map(|p| p.to_path_buf()),
            environment_vars: environment_vars.clone(),
            exit_code: None,
            execution_time_ms: None,
            validation_result: "passed".to_string(),
            security_policy_version: "1.0".to_string(),
        }
    }

    /// Update the audit event with execution results
    pub fn with_execution_result(mut self, exit_code: i32, execution_time_ms: u64) -> Self {
        self.exit_code = Some(exit_code);
        self.execution_time_ms = Some(execution_time_ms);
        self
    }

    /// Update the audit event with validation failure
    pub fn with_validation_failure(mut self, error: &str) -> Self {
        self.validation_result = format!("failed: {error}");
        self
    }
}

/// Global security validator instance
static GLOBAL_VALIDATOR: OnceLock<ShellSecurityValidator> = OnceLock::new();

/// Get or initialize the global security validator
pub fn get_validator() -> &'static ShellSecurityValidator {
    GLOBAL_VALIDATOR.get_or_init(|| {
        // Try to load configuration from SahConfig
        let policy = match load_security_policy() {
            Ok(Some(policy)) => {
                info!(target: "shell_security", "Loaded security policy from configuration");
                policy
            }
            Ok(None) => {
                info!(target: "shell_security", "No security configuration found, using default policy");
                ShellSecurityPolicy::default()
            }
            Err(e) => {
                // In tests, be more graceful about configuration errors
                if cfg!(test) {
                    warn!(target: "shell_security", "Configuration error in test environment: {}. Using default policy.", e);
                    ShellSecurityPolicy::default()
                } else {
                    // This is a critical error - invalid security configuration could be a security risk
                    panic!("Critical security error: {e}. Application cannot start with invalid security configuration.");
                }
            }
        };

        ShellSecurityValidator::new(policy).unwrap_or_else(|e| {
            warn!(
                "Failed to create security validator: {}. Using default policy.",
                e
            );
            ShellSecurityValidator::new(ShellSecurityPolicy::default())
                .expect("Default policy should always work")
        })
    })
}

/// Load security policy from configuration, failing fast on invalid configuration
fn load_security_policy() -> Result<Option<ShellSecurityPolicy>> {
    // Try to load configuration from all sources
    match swissarmyhammer_config::load_configuration() {
        Ok(template_context) => {
            // Try to extract shell security policy from config
            match template_context.get("shell_security") {
                Some(value) => {
                    // The TemplateContext uses serde_json::Value internally, so we can use it directly
                    match serde_json::from_value(value.clone()) {
                        Ok(policy) => Ok(Some(policy)),
                        Err(e) => {
                            let error_msg = format!("Invalid shell security policy configuration: {e}. Security configuration must be valid to prevent security vulnerabilities.");
                            error!(target: "shell_security", "Failed to deserialize shell security policy: {}", e);
                            Err(SwissArmyHammerError::Other { message: error_msg })
                        }
                    }
                }
                None => Ok(None), // No shell_security section is fine
            }
        }
        Err(e) => {
            // Config loading failed - this could indicate corruption or permission issues
            let error_msg = format!("Failed to load configuration: {}. This could indicate a corrupted config file or permission issues.", e);
            error!(target: "shell_security", "Failed to load configuration: {}", e);
            Err(SwissArmyHammerError::Other { message: error_msg })
        }
    }
}

/// Log a shell command execution for audit purposes
pub fn log_shell_execution(
    command: &str,
    working_dir: Option<&Path>,
    environment_vars: &HashMap<String, String>,
) {
    let validator = get_validator();
    if !validator.policy().enable_audit_logging {
        return;
    }

    let audit_event = ShellAuditEvent::new(command.to_string(), working_dir, environment_vars);

    // Log using structured logging
    info!(
        target: "shell_audit",
        command = %audit_event.command,
        working_dir = ?audit_event.working_directory,
        env_count = audit_event.environment_vars.len(),
        timestamp = audit_event.timestamp,
        "Shell command execution started"
    );
}

/// Log shell command completion for audit purposes
pub fn log_shell_completion(command: &str, exit_code: i32, execution_time_ms: u64) {
    let validator = get_validator();
    if !validator.policy().enable_audit_logging {
        return;
    }

    // Log completion with structured logging
    info!(
        target: "shell_audit",
        command = %command,
        exit_code = exit_code,
        execution_time_ms = execution_time_ms,
        success = exit_code == 0,
        "Shell command execution completed"
    );

    // Log security concern if command failed with suspicious exit code
    if exit_code != 0 && exit_code != 1 {
        warn!(
            target: "shell_audit",
            command = %command,
            exit_code = exit_code,
            "Shell command failed with unusual exit code"
        );
    }
}

// Workflow validation functions that were previously in swissarmyhammer::workflow
// These are shell-specific and are now part of the shell domain crate

/// Validate a command for security issues
pub fn validate_command(command: &str) -> std::result::Result<(), ShellSecurityError> {
    get_validator().validate_command(command)
}

/// Validate working directory access security
pub fn validate_working_directory_security(
    directory: &Path,
) -> std::result::Result<(), ShellSecurityError> {
    get_validator().validate_directory_access(directory)
}

/// Validate environment variables security
pub fn validate_environment_variables_security(
    env_vars: &HashMap<String, String>,
) -> std::result::Result<(), ShellSecurityError> {
    get_validator().validate_environment_variables(env_vars)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[test]
    fn test_shell_security_validator_creation() {
        let policy = ShellSecurityPolicy::default();
        let validator = ShellSecurityValidator::new(policy);
        assert!(validator.is_ok());
    }

    #[test]
    fn test_command_length_validation() {
        let policy = ShellSecurityPolicy {
            max_command_length: 10,
            ..Default::default()
        };
        let validator = ShellSecurityValidator::new(policy).unwrap();

        // Valid command
        assert!(validator.validate_command("echo hello").is_ok());

        // Command too long
        let long_command = "a".repeat(11);
        let result = validator.validate_command(&long_command);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ShellSecurityError::CommandTooLong { .. }
        ));
    }

    #[test]
    fn test_shell_constructs_now_allowed() {
        let policy = ShellSecurityPolicy::default();
        let validator = ShellSecurityValidator::new(policy).unwrap();

        // Safe commands should pass
        assert!(validator.validate_command("echo hello").is_ok());
        assert!(validator.validate_command("ls -la").is_ok());

        // Shell constructs should now be allowed (injection patterns removed)
        let allowed_commands = [
            "echo hello; ls",
            "ls | grep test",
            "echo $(date)",
            "echo `whoami`",
            "ls && echo done",
            "echo hello || echo world",
        ];

        for cmd in &allowed_commands {
            let result = validator.validate_command(cmd);
            assert!(result.is_ok(), "Command should be allowed: {cmd}");
        }

        // But commands with blocked patterns should still be blocked
        let blocked_commands = [
            "rm -rf /",        // Matches blocked pattern
            "sudo echo hello", // Matches blocked pattern
        ];

        for cmd in &blocked_commands {
            let result = validator.validate_command(cmd);
            assert!(
                result.is_err(),
                "Command should still be blocked by blocked patterns: {cmd}"
            );
            assert!(matches!(
                result.unwrap_err(),
                ShellSecurityError::BlockedCommandPattern { .. }
            ));
        }
    }

    #[test]
    fn test_blocked_command_patterns() {
        let policy = ShellSecurityPolicy {
            blocked_commands: vec![r"test_blocked".to_string()],
            ..Default::default()
        };
        let validator = ShellSecurityValidator::new(policy).unwrap();

        // Normal command should pass
        assert!(validator.validate_command("echo hello").is_ok());

        // Blocked pattern should fail
        let result = validator.validate_command("test_blocked command");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ShellSecurityError::BlockedCommandPattern { .. }
        ));
    }

    #[test]
    fn test_directory_access_validation() {
        let temp_dir = TempDir::new().unwrap();
        let allowed_path = temp_dir.path();
        let forbidden_path = std::env::temp_dir();

        let policy = ShellSecurityPolicy {
            allowed_directories: Some(vec![allowed_path.to_path_buf()]),
            ..Default::default()
        };
        let validator = ShellSecurityValidator::new(policy).unwrap();

        // Access to allowed directory should succeed
        assert!(validator.validate_directory_access(allowed_path).is_ok());

        // Access to forbidden directory should fail
        let result = validator.validate_directory_access(&forbidden_path);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ShellSecurityError::DirectoryAccessDenied { .. }
        ));
    }

    #[test]
    fn test_environment_variable_validation() {
        let policy = ShellSecurityPolicy::default();
        let validator = ShellSecurityValidator::new(policy).unwrap();

        // Valid environment variables
        let mut valid_env = HashMap::new();
        valid_env.insert("PATH".to_string(), "/usr/bin".to_string());
        valid_env.insert("_UNDERSCORE".to_string(), "value".to_string());
        valid_env.insert("VAR123".to_string(), "value".to_string());
        assert!(validator.validate_environment_variables(&valid_env).is_ok());

        // Invalid environment variable names
        let mut invalid_env = HashMap::new();
        invalid_env.insert("123INVALID".to_string(), "value".to_string()); // Starts with digit
        let result = validator.validate_environment_variables(&invalid_env);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ShellSecurityError::InvalidEnvironmentVariable { .. }
        ));
    }

    #[test]
    fn test_audit_event_creation() {
        let temp_dir = TempDir::new().unwrap();
        let env_vars = HashMap::new();

        let event = ShellAuditEvent::new("echo test".to_string(), Some(temp_dir.path()), &env_vars);

        assert_eq!(event.command, "echo test");
        assert_eq!(event.working_directory, Some(temp_dir.path().to_path_buf()));
        assert_eq!(event.validation_result, "passed");
    }

    #[test]
    fn test_is_valid_env_var_name() {
        assert!(ShellSecurityValidator::is_valid_env_var_name("PATH"));
        assert!(ShellSecurityValidator::is_valid_env_var_name("_UNDERSCORE"));
        assert!(ShellSecurityValidator::is_valid_env_var_name("VAR123"));

        assert!(!ShellSecurityValidator::is_valid_env_var_name("123INVALID"));
        assert!(!ShellSecurityValidator::is_valid_env_var_name(""));
        assert!(!ShellSecurityValidator::is_valid_env_var_name(
            "INVALID-NAME"
        ));
    }

    #[test]
    fn test_policy_disabled_validation() {
        let policy = ShellSecurityPolicy {
            enable_validation: false,
            ..Default::default()
        };
        let validator = ShellSecurityValidator::new(policy).unwrap();

        // Even dangerous commands should pass when validation is disabled
        assert!(validator.validate_command("echo hello; rm -rf /").is_ok());
    }

    #[test]
    fn test_workflow_validation_functions() {
        // Test the workflow validation functions that are now part of this crate
        assert!(validate_command("echo hello").is_ok());

        let temp_dir = TempDir::new().unwrap();
        assert!(validate_working_directory_security(temp_dir.path()).is_ok());

        let env_vars = HashMap::new();
        assert!(validate_environment_variables_security(&env_vars).is_ok());
    }

    #[test]
    fn test_path_traversal_detection() {
        // Test that path traversal attempts are blocked
        use std::path::Path;

        let dangerous_paths = ["../parent", "path/../parent", "/absolute/../parent"];

        for path in &dangerous_paths {
            let result = validate_working_directory_security(Path::new(path));
            assert!(
                result.is_err(),
                "Path traversal attempt '{}' should be blocked",
                path
            );

            // Also test through the validator directly
            let validator = get_validator();
            let result2 = validator.validate_directory_access(Path::new(path));
            assert!(
                result2.is_err(),
                "Direct validator call for '{}' should also be blocked",
                path
            );
        }
    }
}
