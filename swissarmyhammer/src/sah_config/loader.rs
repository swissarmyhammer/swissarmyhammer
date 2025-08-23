use crate::sah_config::{
    env_vars::{EnvVarError, EnvVarProcessor},
    types::{parse_size_string, CacheMetadata, ConfigValue, Configuration, ShellToolConfig},
    validation::{ValidationError, Validator},
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use thiserror::Error;

/// Errors that can occur when loading configuration files
#[derive(Error, Debug)]
pub enum ConfigurationError {
    /// IO error occurred while reading the configuration file
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// TOML parsing error occurred while parsing the configuration
    #[error("TOML parsing error: {0}")]
    TomlParse(#[from] toml::de::Error),

    /// Configuration file exceeds maximum allowed size
    #[error("File is too large: {size} bytes (max: {max_size} bytes)")]
    FileTooLarge {
        /// Actual file size in bytes
        size: u64,
        /// Maximum allowed size in bytes
        max_size: u64,
    },

    /// Configuration file was not found at the specified path
    #[error("Configuration file not found: {path}")]
    FileNotFound {
        /// Path where the file was expected
        path: String,
    },

    /// File contains invalid UTF-8 characters
    #[error("Invalid UTF-8 in configuration file")]
    InvalidUtf8,

    /// Error occurred during environment variable substitution
    #[error("Environment variable substitution error: {0}")]
    EnvVarSubstitution(#[from] EnvVarError),

    /// Validation error occurred during configuration validation
    #[error("Configuration validation error: {0}")]
    Validation(#[from] ValidationError),

    /// Invalid shell configuration value
    #[error("Invalid shell configuration value for {key}: {value} - {reason}")]
    InvalidShellValue {
        /// Configuration key that failed validation
        key: String,
        /// Invalid value that was provided
        value: String,
        /// Reason why the value is invalid
        reason: String,
    },
}

/// Configuration loader that handles file loading and TOML parsing
pub struct ConfigurationLoader {
    max_file_size: u64,
    validator: Validator,
    env_processor: EnvVarProcessor,
    enable_env_substitution: bool,
    enable_validation: bool,
}

impl ConfigurationLoader {
    /// Create a new configuration loader with default settings
    pub fn new() -> Result<Self, ConfigurationError> {
        Ok(Self {
            max_file_size: 1024 * 1024, // 1MB default limit
            validator: Validator::new(),
            env_processor: EnvVarProcessor::new()?,
            enable_env_substitution: true,
            enable_validation: true,
        })
    }

    /// Create a configuration loader with custom file size limit
    pub fn with_max_file_size(max_file_size: u64) -> Result<Self, ConfigurationError> {
        Ok(Self {
            max_file_size,
            validator: Validator::new(),
            env_processor: EnvVarProcessor::new()?,
            enable_env_substitution: true,
            enable_validation: true,
        })
    }

    /// Enable or disable environment variable substitution
    pub fn with_env_substitution(mut self, enable: bool) -> Self {
        self.enable_env_substitution = enable;
        self
    }

    /// Enable or disable validation
    pub fn with_validation(mut self, enable: bool) -> Self {
        self.enable_validation = enable;
        self
    }

    /// Use a custom validator
    pub fn with_validator(mut self, validator: Validator) -> Self {
        self.validator = validator;
        self
    }

    /// Load configuration from a specific file path
    pub fn load_from_file(&self, file_path: &Path) -> Result<Configuration, ConfigurationError> {
        // Security validation - check file path and permissions
        if self.enable_validation {
            self.validator.validate_file_security(file_path)?;
        }

        if !file_path.exists() {
            return Err(ConfigurationError::FileNotFound {
                path: file_path.to_string_lossy().to_string(),
            });
        }

        // Check file size
        let metadata = fs::metadata(file_path)?;
        if metadata.len() > self.max_file_size {
            return Err(ConfigurationError::FileTooLarge {
                size: metadata.len(),
                max_size: self.max_file_size,
            });
        }

        // Read file contents with UTF-8 validation
        let contents = self.read_file_contents_securely(file_path)?;

        // Parse TOML
        let toml_value: toml::Value = toml::from_str(&contents)?;

        // Convert to our ConfigValue representation
        let mut config_values = self.toml_value_to_config_map(toml_value)?;

        // Process environment variable substitution
        if self.enable_env_substitution {
            self.process_env_substitution(&mut config_values)?;
        }

        // Create cache metadata
        let cache_metadata = CacheMetadata::from_file(file_path).ok();

        // Create configuration
        let config = Configuration::with_cache_metadata(
            config_values,
            Some(file_path.to_path_buf()),
            cache_metadata,
        );

        // Validate configuration
        if self.enable_validation {
            self.validator.validate(&config)?;
        }

        Ok(config)
    }

    /// Load configuration from repository root
    ///
    /// Searches for sah.toml starting from current directory and walking up
    /// to find the repository root (indicated by .git directory or filesystem root)
    /// Returns None if not within a git repository.
    pub fn load_from_repo_root(&self) -> Result<Option<Configuration>, ConfigurationError> {
        let current_dir = std::env::current_dir().map_err(ConfigurationError::Io)?;
        let mut search_dir = current_dir.as_path();

        // First, find the repository root
        let mut repo_root = None;
        let mut check_dir = search_dir;
        loop {
            if check_dir.join(".git").exists() {
                repo_root = Some(check_dir);
                break;
            }
            match check_dir.parent() {
                Some(parent) => check_dir = parent,
                None => break, // Reached filesystem root
            }
        }

        // If not in a repository, return None
        let repo_root = match repo_root {
            Some(root) => root,
            None => return Ok(None),
        };

        // Now search for sah.toml from current directory up to repository root
        search_dir = current_dir.as_path();
        loop {
            let sah_toml_path = search_dir.join("sah.toml");

            if sah_toml_path.exists() {
                return Ok(Some(self.load_from_file(&sah_toml_path)?));
            }

            // Stop when we reach the repository root
            if search_dir == repo_root {
                break;
            }

            // Move to parent directory
            match search_dir.parent() {
                Some(parent) => search_dir = parent,
                None => break, // Reached filesystem root (shouldn't happen)
            }
        }

        Ok(None)
    }

    /// Convert a TOML value to our ConfigValue representation
    fn toml_value_to_config_value(value: toml::Value) -> Result<ConfigValue, ConfigurationError> {
        match value {
            toml::Value::String(s) => Ok(ConfigValue::String(s)),
            toml::Value::Integer(i) => Ok(ConfigValue::Integer(i)),
            toml::Value::Float(f) => Ok(ConfigValue::Float(f)),
            toml::Value::Boolean(b) => Ok(ConfigValue::Boolean(b)),
            toml::Value::Array(arr) => {
                let mut config_array = Vec::new();
                for item in arr {
                    config_array.push(Self::toml_value_to_config_value(item)?);
                }
                Ok(ConfigValue::Array(config_array))
            }
            toml::Value::Table(table) => {
                let mut config_table = HashMap::new();
                for (key, value) in table {
                    config_table.insert(key, Self::toml_value_to_config_value(value)?);
                }
                Ok(ConfigValue::Table(config_table))
            }
            toml::Value::Datetime(_) => {
                // Convert datetime to string representation
                Ok(ConfigValue::String(value.to_string()))
            }
        }
    }

    /// Convert a TOML value to a map of configuration values
    fn toml_value_to_config_map(
        &self,
        value: toml::Value,
    ) -> Result<HashMap<String, ConfigValue>, ConfigurationError> {
        match value {
            toml::Value::Table(table) => {
                let mut config_map = HashMap::new();
                for (key, value) in table {
                    config_map.insert(key, Self::toml_value_to_config_value(value)?);
                }
                Ok(config_map)
            }
            _ => {
                // If the root is not a table, create a single entry
                let mut config_map = HashMap::new();
                config_map.insert(
                    "value".to_string(),
                    Self::toml_value_to_config_value(value)?,
                );
                Ok(config_map)
            }
        }
    }

    /// Securely read file contents with UTF-8 validation
    fn read_file_contents_securely(&self, file_path: &Path) -> Result<String, ConfigurationError> {
        // Read as bytes first to provide better error information
        let bytes = fs::read(file_path)?;

        // Validate UTF-8 encoding
        match String::from_utf8(bytes) {
            Ok(content) => Ok(content),
            Err(_) => Err(ConfigurationError::InvalidUtf8),
        }
    }

    /// Process environment variable substitution in configuration values
    fn process_env_substitution(
        &self,
        values: &mut HashMap<String, ConfigValue>,
    ) -> Result<(), ConfigurationError> {
        for value in values.values_mut() {
            self.process_env_substitution_recursive(value)?;
        }
        Ok(())
    }

    /// Recursively process environment variable substitution in nested structures
    fn process_env_substitution_recursive(
        &self,
        value: &mut ConfigValue,
    ) -> Result<(), ConfigurationError> {
        match value {
            ConfigValue::String(s) => {
                *s = self.env_processor.substitute_variables(s)?;
            }
            ConfigValue::Array(arr) => {
                for item in arr {
                    self.process_env_substitution_recursive(item)?;
                }
            }
            ConfigValue::Table(table) => {
                for table_value in table.values_mut() {
                    self.process_env_substitution_recursive(table_value)?;
                }
            }
            _ => {} // Other types don't need environment variable substitution
        }
        Ok(())
    }

    /// Load shell tool configuration from repository root
    pub fn load_shell_config(&self) -> Result<ShellToolConfig, ConfigurationError> {
        // Start with default configuration
        let mut config = ShellToolConfig::default();

        // Load from configuration file if it exists
        if let Some(file_config) = self.load_from_repo_root()? {
            config = self.merge_shell_config(config, &file_config)?;
        }

        // Override with environment variables
        config = self.apply_shell_env_overrides(config)?;

        // Validate configuration
        self.validate_shell_config(&config)?;

        Ok(config)
    }

    /// Merge shell configuration from loaded file configuration
    fn merge_shell_config(
        &self,
        mut config: ShellToolConfig,
        file_config: &Configuration,
    ) -> Result<ShellToolConfig, ConfigurationError> {
        // Look for shell configuration section
        if let Some(ConfigValue::Table(shell_table)) = file_config.get("shell") {
            // Merge security settings
            if let Some(ConfigValue::Table(security_table)) = shell_table.get("security") {
                self.merge_shell_security_config(&mut config.security, security_table)?;
            }

            // Merge output settings
            if let Some(ConfigValue::Table(output_table)) = shell_table.get("output") {
                self.merge_shell_output_config(&mut config.output, output_table)?;
            }

            // Merge execution settings
            if let Some(ConfigValue::Table(execution_table)) = shell_table.get("execution") {
                self.merge_shell_execution_config(&mut config.execution, execution_table)?;
            }

            // Merge audit settings
            if let Some(ConfigValue::Table(audit_table)) = shell_table.get("audit") {
                self.merge_shell_audit_config(&mut config.audit, audit_table)?;
            }
        }

        Ok(config)
    }

    /// Merge security configuration from TOML values
    fn merge_shell_security_config(
        &self,
        config: &mut crate::sah_config::types::ShellSecurityConfig,
        table: &HashMap<String, ConfigValue>,
    ) -> Result<(), ConfigurationError> {
        if let Some(ConfigValue::Boolean(enable)) = table.get("enable_validation") {
            config.enable_validation = *enable;
        }

        if let Some(ConfigValue::Array(commands)) = table.get("blocked_commands") {
            config.blocked_commands = commands
                .iter()
                .filter_map(|v| match v {
                    ConfigValue::String(s) => Some(s.clone()),
                    _ => None,
                })
                .collect();
        }

        if let Some(ConfigValue::Array(dirs)) = table.get("allowed_directories") {
            config.allowed_directories = Some(
                dirs.iter()
                    .filter_map(|v| match v {
                        ConfigValue::String(s) => Some(s.clone()),
                        _ => None,
                    })
                    .collect(),
            );
        }

        if let Some(ConfigValue::Integer(length)) = table.get("max_command_length") {
            config.max_command_length = *length as usize;
        }

        if let Some(ConfigValue::Boolean(enable)) = table.get("enable_injection_detection") {
            config.enable_injection_detection = *enable;
        }

        Ok(())
    }

    /// Merge output configuration from TOML values
    fn merge_shell_output_config(
        &self,
        config: &mut crate::sah_config::types::ShellOutputConfig,
        table: &HashMap<String, ConfigValue>,
    ) -> Result<(), ConfigurationError> {
        if let Some(ConfigValue::String(size)) = table.get("max_output_size") {
            config.max_output_size = size.clone();
        }

        if let Some(ConfigValue::Integer(length)) = table.get("max_line_length") {
            config.max_line_length = *length as usize;
        }

        if let Some(ConfigValue::Boolean(detect)) = table.get("detect_binary_content") {
            config.detect_binary_content = *detect;
        }

        if let Some(ConfigValue::String(strategy)) = table.get("truncation_strategy") {
            use crate::sah_config::types::TruncationStrategy;
            config.truncation_strategy =
                match strategy.as_str() {
                    "preserve_structure" => TruncationStrategy::PreserveStructure,
                    "simple_truncation" => TruncationStrategy::SimpleTruncation,
                    "word_boundary" => TruncationStrategy::WordBoundary,
                    _ => return Err(ConfigurationError::InvalidShellValue {
                        key: "truncation_strategy".to_string(),
                        value: strategy.clone(),
                        reason:
                            "Must be one of: preserve_structure, simple_truncation, word_boundary"
                                .to_string(),
                    }),
                };
        }

        Ok(())
    }

    /// Merge execution configuration from TOML values
    fn merge_shell_execution_config(
        &self,
        config: &mut crate::sah_config::types::ShellExecutionConfig,
        table: &HashMap<String, ConfigValue>,
    ) -> Result<(), ConfigurationError> {
        if let Some(ConfigValue::Integer(timeout)) = table.get("default_timeout") {
            config.default_timeout = *timeout as u64;
        }

        if let Some(ConfigValue::Integer(timeout)) = table.get("max_timeout") {
            config.max_timeout = *timeout as u64;
        }

        if let Some(ConfigValue::Integer(timeout)) = table.get("min_timeout") {
            config.min_timeout = *timeout as u64;
        }

        if let Some(ConfigValue::Boolean(cleanup)) = table.get("cleanup_process_tree") {
            config.cleanup_process_tree = *cleanup;
        }

        Ok(())
    }

    /// Merge audit configuration from TOML values
    fn merge_shell_audit_config(
        &self,
        config: &mut crate::sah_config::types::ShellAuditConfig,
        table: &HashMap<String, ConfigValue>,
    ) -> Result<(), ConfigurationError> {
        if let Some(ConfigValue::Boolean(enable)) = table.get("enable_audit_logging") {
            config.enable_audit_logging = *enable;
        }

        if let Some(ConfigValue::String(level)) = table.get("log_level") {
            config.log_level = level.clone();
        }

        if let Some(ConfigValue::Boolean(log_output)) = table.get("log_command_output") {
            config.log_command_output = *log_output;
        }

        if let Some(ConfigValue::Integer(size)) = table.get("max_audit_entry_size") {
            config.max_audit_entry_size = *size as usize;
        }

        Ok(())
    }

    /// Apply environment variable overrides for shell configuration
    fn apply_shell_env_overrides(
        &self,
        mut config: ShellToolConfig,
    ) -> Result<ShellToolConfig, ConfigurationError> {
        use std::env;

        // Security overrides
        if let Ok(val) = env::var("SAH_SHELL_SECURITY_ENABLE_VALIDATION") {
            config.security.enable_validation =
                val.parse()
                    .map_err(|_| ConfigurationError::InvalidShellValue {
                        key: "SAH_SHELL_SECURITY_ENABLE_VALIDATION".to_string(),
                        value: val,
                        reason: "Must be 'true' or 'false'".to_string(),
                    })?;
        }

        if let Ok(val) = env::var("SAH_SHELL_SECURITY_MAX_COMMAND_LENGTH") {
            config.security.max_command_length =
                val.parse()
                    .map_err(|_| ConfigurationError::InvalidShellValue {
                        key: "SAH_SHELL_SECURITY_MAX_COMMAND_LENGTH".to_string(),
                        value: val,
                        reason: "Must be a positive integer".to_string(),
                    })?;
        }

        // Output overrides
        if let Ok(val) = env::var("SAH_SHELL_OUTPUT_MAX_SIZE") {
            // Validate the size string format
            parse_size_string(&val).map_err(|reason| ConfigurationError::InvalidShellValue {
                key: "SAH_SHELL_OUTPUT_MAX_SIZE".to_string(),
                value: val.clone(),
                reason,
            })?;
            config.output.max_output_size = val;
        }

        if let Ok(val) = env::var("SAH_SHELL_OUTPUT_MAX_LINE_LENGTH") {
            config.output.max_line_length =
                val.parse()
                    .map_err(|_| ConfigurationError::InvalidShellValue {
                        key: "SAH_SHELL_OUTPUT_MAX_LINE_LENGTH".to_string(),
                        value: val,
                        reason: "Must be a positive integer".to_string(),
                    })?;
        }

        // Execution overrides
        if let Ok(val) = env::var("SAH_SHELL_EXECUTION_DEFAULT_TIMEOUT") {
            config.execution.default_timeout =
                val.parse()
                    .map_err(|_| ConfigurationError::InvalidShellValue {
                        key: "SAH_SHELL_EXECUTION_DEFAULT_TIMEOUT".to_string(),
                        value: val,
                        reason: "Must be a positive integer (seconds)".to_string(),
                    })?;
        }

        if let Ok(val) = env::var("SAH_SHELL_EXECUTION_MAX_TIMEOUT") {
            config.execution.max_timeout =
                val.parse()
                    .map_err(|_| ConfigurationError::InvalidShellValue {
                        key: "SAH_SHELL_EXECUTION_MAX_TIMEOUT".to_string(),
                        value: val,
                        reason: "Must be a positive integer (seconds)".to_string(),
                    })?;
        }

        // Audit overrides
        if let Ok(val) = env::var("SAH_SHELL_AUDIT_ENABLE_LOGGING") {
            config.audit.enable_audit_logging =
                val.parse()
                    .map_err(|_| ConfigurationError::InvalidShellValue {
                        key: "SAH_SHELL_AUDIT_ENABLE_LOGGING".to_string(),
                        value: val,
                        reason: "Must be 'true' or 'false'".to_string(),
                    })?;
        }

        if let Ok(val) = env::var("SAH_SHELL_AUDIT_LOG_LEVEL") {
            // Validate log level
            match val.to_lowercase().as_str() {
                "trace" | "debug" | "info" | "warn" | "error" => {
                    config.audit.log_level = val.to_lowercase();
                }
                _ => {
                    return Err(ConfigurationError::InvalidShellValue {
                        key: "SAH_SHELL_AUDIT_LOG_LEVEL".to_string(),
                        value: val,
                        reason: "Must be one of: trace, debug, info, warn, error".to_string(),
                    })
                }
            }
        }

        Ok(config)
    }

    /// Validate shell configuration for consistency and security
    fn validate_shell_config(&self, config: &ShellToolConfig) -> Result<(), ConfigurationError> {
        // Validate timeout ranges
        if config.execution.default_timeout < config.execution.min_timeout {
            return Err(ConfigurationError::InvalidShellValue {
                key: "execution.default_timeout".to_string(),
                value: config.execution.default_timeout.to_string(),
                reason: "Default timeout cannot be less than minimum timeout".to_string(),
            });
        }

        if config.execution.default_timeout > config.execution.max_timeout {
            return Err(ConfigurationError::InvalidShellValue {
                key: "execution.default_timeout".to_string(),
                value: config.execution.default_timeout.to_string(),
                reason: "Default timeout cannot exceed maximum timeout".to_string(),
            });
        }

        // Validate output size format
        parse_size_string(&config.output.max_output_size).map_err(|reason| {
            ConfigurationError::InvalidShellValue {
                key: "output.max_output_size".to_string(),
                value: config.output.max_output_size.clone(),
                reason,
            }
        })?;

        // Validate command length is reasonable
        if config.security.max_command_length == 0 {
            return Err(ConfigurationError::InvalidShellValue {
                key: "security.max_command_length".to_string(),
                value: config.security.max_command_length.to_string(),
                reason: "Command length must be greater than 0".to_string(),
            });
        }

        if config.security.max_command_length > 100_000 {
            return Err(ConfigurationError::InvalidShellValue {
                key: "security.max_command_length".to_string(),
                value: config.security.max_command_length.to_string(),
                reason: "Command length cannot exceed 100,000 characters".to_string(),
            });
        }

        // Validate output line length is reasonable
        if config.output.max_line_length == 0 {
            return Err(ConfigurationError::InvalidShellValue {
                key: "output.max_line_length".to_string(),
                value: config.output.max_line_length.to_string(),
                reason: "Line length must be greater than 0".to_string(),
            });
        }

        // Validate audit entry size is reasonable
        if config.audit.max_audit_entry_size == 0 {
            return Err(ConfigurationError::InvalidShellValue {
                key: "audit.max_audit_entry_size".to_string(),
                value: config.audit.max_audit_entry_size.to_string(),
                reason: "Audit entry size must be greater than 0".to_string(),
            });
        }

        Ok(())
    }
}

impl Default for ConfigurationLoader {
    fn default() -> Self {
        Self::new().expect("Failed to create default ConfigurationLoader")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_valid_toml() -> Result<(), Box<dyn std::error::Error>> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(
            temp_file,
            r#"
project_name = "Test Project"
version = 123
debug = true
servers = ["server1", "server2"]

[database]
host = "localhost"
port = 5432
"#
        )?;

        let loader = ConfigurationLoader::new()?.with_validation(false);
        let config = loader.load_from_file(temp_file.path())?;

        assert_eq!(config.len(), 5);
        assert_eq!(
            config.get("project_name"),
            Some(&ConfigValue::String("Test Project".to_string()))
        );
        assert_eq!(config.get("version"), Some(&ConfigValue::Integer(123)));
        assert_eq!(config.get("debug"), Some(&ConfigValue::Boolean(true)));

        // Check nested table
        if let Some(ConfigValue::Table(db_table)) = config.get("database") {
            assert_eq!(
                db_table.get("host"),
                Some(&ConfigValue::String("localhost".to_string()))
            );
            assert_eq!(db_table.get("port"), Some(&ConfigValue::Integer(5432)));
        } else {
            panic!("Expected database table");
        }

        Ok(())
    }

    #[test]
    fn test_file_not_found() -> Result<(), Box<dyn std::error::Error>> {
        let loader = ConfigurationLoader::new()?;
        let result = loader.load_from_file(Path::new("nonexistent.toml"));

        assert!(matches!(
            result,
            Err(ConfigurationError::FileNotFound { .. })
        ));
        Ok(())
    }

    #[test]
    fn test_file_too_large() -> Result<(), Box<dyn std::error::Error>> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "test = 'value'")?;

        let loader = ConfigurationLoader::with_max_file_size(5)?.with_validation(false); // Very small limit
        let result = loader.load_from_file(temp_file.path());

        assert!(matches!(
            result,
            Err(ConfigurationError::FileTooLarge { .. })
        ));
        Ok(())
    }

    #[test]
    fn test_invalid_toml() -> Result<(), Box<dyn std::error::Error>> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "invalid toml content [")?;

        let loader = ConfigurationLoader::new()?.with_validation(false);
        let result = loader.load_from_file(temp_file.path());

        assert!(matches!(result, Err(ConfigurationError::TomlParse(_))));
        Ok(())
    }

    #[test]
    fn test_toml_value_conversion() -> Result<(), Box<dyn std::error::Error>> {
        let _loader = ConfigurationLoader::new()?;

        // Test string conversion
        let string_val = toml::Value::String("test".to_string());
        let config_val = ConfigurationLoader::toml_value_to_config_value(string_val)?;
        assert_eq!(config_val, ConfigValue::String("test".to_string()));

        // Test integer conversion
        let int_val = toml::Value::Integer(42);
        let config_val = ConfigurationLoader::toml_value_to_config_value(int_val)?;
        assert_eq!(config_val, ConfigValue::Integer(42));

        // Test boolean conversion
        let bool_val = toml::Value::Boolean(true);
        let config_val = ConfigurationLoader::toml_value_to_config_value(bool_val)?;
        assert_eq!(config_val, ConfigValue::Boolean(true));

        Ok(())
    }

    #[test]
    fn test_environment_variable_substitution() -> Result<(), Box<dyn std::error::Error>> {
        use crate::test_utils::IsolatedTestHome;
        use std::env;

        // Use isolated test home to avoid interfering with other tests
        let _guard = IsolatedTestHome::new();

        // Set up test environment variables
        env::set_var("TEST_DB_HOST", "production.example.com");
        env::set_var("TEST_DB_PORT", "5432");

        let mut temp_file = NamedTempFile::new()?;
        write!(
            temp_file,
            r#"
database_url = "postgresql://${{TEST_DB_HOST}}:${{TEST_DB_PORT}}/myapp"
api_key = "${{TEST_API_KEY:-default_key}}"
debug = "${{TEST_DEBUG:-false}}"
"#
        )?;

        let loader = ConfigurationLoader::new()?
            .with_env_substitution(true)
            .with_validation(false);
        let config = loader.load_from_file(temp_file.path())?;

        // Check that environment variables were substituted
        if let Some(ConfigValue::String(db_url)) = config.get("database_url") {
            assert_eq!(db_url, "postgresql://production.example.com:5432/myapp");
        } else {
            panic!("Expected database_url to be a string");
        }

        if let Some(ConfigValue::String(api_key)) = config.get("api_key") {
            assert_eq!(api_key, "default_key"); // Should use default since TEST_API_KEY is not set
        } else {
            panic!("Expected api_key to be a string");
        }

        // Clean up
        env::remove_var("TEST_DB_HOST");
        env::remove_var("TEST_DB_PORT");

        Ok(())
    }

    #[test]
    fn test_validation_disabled() -> Result<(), Box<dyn std::error::Error>> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(
            temp_file,
            r#"
# This would normally fail validation due to reserved name
for = "invalid_reserved_name"
"#
        )?;

        // With validation disabled, this should work
        let loader = ConfigurationLoader::new()?.with_validation(false);
        let config = loader.load_from_file(temp_file.path())?;

        assert_eq!(config.len(), 1);
        assert_eq!(
            config.get("for"),
            Some(&ConfigValue::String("invalid_reserved_name".to_string()))
        );

        Ok(())
    }

    #[test]
    fn test_validation_enabled() -> Result<(), Box<dyn std::error::Error>> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(
            temp_file,
            r#"
# This should fail validation due to reserved name
for = "invalid_reserved_name"
"#
        )?;

        // With validation enabled (default), this should fail
        let loader = ConfigurationLoader::new()?;
        let result = loader.load_from_file(temp_file.path());

        assert!(matches!(result, Err(ConfigurationError::Validation(_))));

        Ok(())
    }

    #[test]
    fn test_cache_metadata_creation() -> Result<(), Box<dyn std::error::Error>> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "test = 'value'")?;

        let loader = ConfigurationLoader::new()?.with_validation(false);
        let config = loader.load_from_file(temp_file.path())?;

        // Should have cache metadata
        assert!(config.cache_metadata().is_some());

        // Should have file path
        assert!(config.file_path().is_some());
        assert_eq!(config.file_path().unwrap(), temp_file.path());

        Ok(())
    }

    #[test]
    fn test_security_validation() -> Result<(), Box<dyn std::error::Error>> {
        // Test with path traversal attempt (should fail with validation enabled)
        let loader = ConfigurationLoader::new()?;
        let result = loader.load_from_file(Path::new("../../../etc/passwd"));

        // Should fail due to security validation
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_loader_configuration_options() -> Result<(), Box<dyn std::error::Error>> {
        // Test chaining configuration methods
        let loader = ConfigurationLoader::with_max_file_size(2048)?
            .with_env_substitution(false)
            .with_validation(false);

        // Create a simple test file
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "test = 'value'")?;

        let config = loader.load_from_file(temp_file.path())?;
        assert_eq!(config.len(), 1);

        Ok(())
    }

    #[test]
    fn test_load_shell_config_defaults() -> Result<(), Box<dyn std::error::Error>> {
        let _guard = crate::test_utils::IsolatedTestEnvironment::new().unwrap();
        let loader = ConfigurationLoader::new()?.with_validation(false);
        let config = loader.load_shell_config()?;

        // Should use default values when no configuration file exists
        assert!(config.security.enable_validation);
        assert_eq!(config.output.max_output_size, "10MB");
        assert_eq!(config.execution.default_timeout, 300);
        assert!(!config.audit.enable_audit_logging);

        Ok(())
    }

    #[test]
    fn test_load_shell_config_from_file() -> Result<(), Box<dyn std::error::Error>> {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new()?;
        writeln!(
            temp_file,
            r#"
[shell.security]
enable_validation = false
blocked_commands = ["rm", "format"]
max_command_length = 2000

[shell.output]
max_output_size = "50MB"
max_line_length = 5000

[shell.execution]
default_timeout = 600
max_timeout = 3600

[shell.audit]
enable_audit_logging = true
log_level = "debug"
"#
        )?;

        let loader = ConfigurationLoader::new()?.with_validation(false);
        let file_config = loader.load_from_file(temp_file.path())?;
        let config = loader.merge_shell_config(ShellToolConfig::default(), &file_config)?;

        // Check that file values override defaults
        assert!(!config.security.enable_validation);
        assert_eq!(config.security.blocked_commands, vec!["rm", "format"]);
        assert_eq!(config.security.max_command_length, 2000);
        assert_eq!(config.output.max_output_size, "50MB");
        assert_eq!(config.output.max_line_length, 5000);
        assert_eq!(config.execution.default_timeout, 600);
        assert_eq!(config.execution.max_timeout, 3600);
        assert!(config.audit.enable_audit_logging);
        assert_eq!(config.audit.log_level, "debug");

        Ok(())
    }

    #[test]
    fn test_shell_env_overrides() -> Result<(), Box<dyn std::error::Error>> {
        use crate::test_utils::IsolatedTestHome;
        use std::env;

        // Use isolated test home to avoid interfering with other tests
        let _guard = IsolatedTestHome::new();

        // Save original env var values for cleanup
        let original_validation = env::var("SAH_SHELL_SECURITY_ENABLE_VALIDATION").ok();
        let original_max_size = env::var("SAH_SHELL_OUTPUT_MAX_SIZE").ok();
        let original_timeout = env::var("SAH_SHELL_EXECUTION_DEFAULT_TIMEOUT").ok();
        let original_logging = env::var("SAH_SHELL_AUDIT_ENABLE_LOGGING").ok();

        // Ensure cleanup happens even if test panics
        struct EnvCleanup {
            validation: Option<String>,
            max_size: Option<String>,
            timeout: Option<String>,
            logging: Option<String>,
        }
        impl Drop for EnvCleanup {
            fn drop(&mut self) {
                match &self.validation {
                    Some(val) => env::set_var("SAH_SHELL_SECURITY_ENABLE_VALIDATION", val),
                    None => env::remove_var("SAH_SHELL_SECURITY_ENABLE_VALIDATION"),
                }
                match &self.max_size {
                    Some(val) => env::set_var("SAH_SHELL_OUTPUT_MAX_SIZE", val),
                    None => env::remove_var("SAH_SHELL_OUTPUT_MAX_SIZE"),
                }
                match &self.timeout {
                    Some(val) => env::set_var("SAH_SHELL_EXECUTION_DEFAULT_TIMEOUT", val),
                    None => env::remove_var("SAH_SHELL_EXECUTION_DEFAULT_TIMEOUT"),
                }
                match &self.logging {
                    Some(val) => env::set_var("SAH_SHELL_AUDIT_ENABLE_LOGGING", val),
                    None => env::remove_var("SAH_SHELL_AUDIT_ENABLE_LOGGING"),
                }
            }
        }
        let _cleanup = EnvCleanup {
            validation: original_validation,
            max_size: original_max_size,
            timeout: original_timeout,
            logging: original_logging,
        };

        // Set test environment variables
        env::set_var("SAH_SHELL_SECURITY_ENABLE_VALIDATION", "false");
        env::set_var("SAH_SHELL_OUTPUT_MAX_SIZE", "5MB");
        env::set_var("SAH_SHELL_EXECUTION_DEFAULT_TIMEOUT", "120");
        env::set_var("SAH_SHELL_AUDIT_ENABLE_LOGGING", "true");

        let loader = ConfigurationLoader::new()?.with_validation(false);
        let config = loader.apply_shell_env_overrides(ShellToolConfig::default())?;

        // Check that environment variables override defaults
        assert!(!config.security.enable_validation);
        assert_eq!(config.output.max_output_size, "5MB");
        assert_eq!(config.execution.default_timeout, 120);
        assert!(config.audit.enable_audit_logging);

        // Cleanup will happen automatically via Drop

        Ok(())
    }

    #[test]
    fn test_shell_config_validation_success() -> Result<(), Box<dyn std::error::Error>> {
        let loader = ConfigurationLoader::new()?.with_validation(false);
        let config = ShellToolConfig::default();

        // Default configuration should be valid
        loader.validate_shell_config(&config)?;

        Ok(())
    }

    #[test]
    fn test_shell_config_validation_timeout_errors() -> Result<(), Box<dyn std::error::Error>> {
        let loader = ConfigurationLoader::new()?.with_validation(false);

        // Test default timeout less than min timeout
        let mut config = ShellToolConfig::default();
        config.execution.default_timeout = 1;
        config.execution.min_timeout = 5;
        assert!(loader.validate_shell_config(&config).is_err());

        // Test default timeout greater than max timeout
        let mut config = ShellToolConfig::default();
        config.execution.default_timeout = 2000;
        config.execution.max_timeout = 1800;
        assert!(loader.validate_shell_config(&config).is_err());

        Ok(())
    }

    #[test]
    fn test_shell_config_validation_invalid_size() -> Result<(), Box<dyn std::error::Error>> {
        let loader = ConfigurationLoader::new()?.with_validation(false);

        // Test invalid size string
        let mut config = ShellToolConfig::default();
        config.output.max_output_size = "invalid_size".to_string();
        assert!(loader.validate_shell_config(&config).is_err());

        Ok(())
    }

    #[test]
    fn test_shell_config_validation_zero_values() -> Result<(), Box<dyn std::error::Error>> {
        let loader = ConfigurationLoader::new()?.with_validation(false);

        // Test zero command length
        let mut config = ShellToolConfig::default();
        config.security.max_command_length = 0;
        assert!(loader.validate_shell_config(&config).is_err());

        // Test zero line length
        let mut config = ShellToolConfig::default();
        config.output.max_line_length = 0;
        assert!(loader.validate_shell_config(&config).is_err());

        // Test zero audit entry size
        let mut config = ShellToolConfig::default();
        config.audit.max_audit_entry_size = 0;
        assert!(loader.validate_shell_config(&config).is_err());

        Ok(())
    }

    #[test]
    fn test_shell_config_validation_excessive_command_length(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let loader = ConfigurationLoader::new()?.with_validation(false);

        let mut config = ShellToolConfig::default();
        config.security.max_command_length = 150_000; // Over 100,000 limit
        assert!(loader.validate_shell_config(&config).is_err());

        Ok(())
    }

    #[test]
    fn test_truncation_strategy_parsing() -> Result<(), Box<dyn std::error::Error>> {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new()?;
        writeln!(
            temp_file,
            r#"
[shell.output]
truncation_strategy = "word_boundary"
"#
        )?;

        let loader = ConfigurationLoader::new()?.with_validation(false);
        let file_config = loader.load_from_file(temp_file.path())?;
        let config = loader.merge_shell_config(ShellToolConfig::default(), &file_config)?;

        assert_eq!(
            config.output.truncation_strategy,
            crate::sah_config::types::TruncationStrategy::WordBoundary
        );

        Ok(())
    }

    #[test]
    fn test_invalid_truncation_strategy() -> Result<(), Box<dyn std::error::Error>> {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new()?;
        writeln!(
            temp_file,
            r#"
[shell.output]
truncation_strategy = "invalid_strategy"
"#
        )?;

        let loader = ConfigurationLoader::new()?.with_validation(false);
        let file_config = loader.load_from_file(temp_file.path())?;
        let result = loader.merge_shell_config(ShellToolConfig::default(), &file_config);

        assert!(result.is_err());
        if let Err(ConfigurationError::InvalidShellValue { key, reason, .. }) = result {
            assert_eq!(key, "truncation_strategy");
            assert!(reason.contains("preserve_structure"));
        }

        Ok(())
    }

    #[test]
    fn test_invalid_env_values() -> Result<(), Box<dyn std::error::Error>> {
        use crate::test_utils::IsolatedTestEnvironment;
        use std::env;

        // Use isolated test environment to avoid interfering with other tests
        let _guard = IsolatedTestEnvironment::new().unwrap();

        // Save original env var values
        let original_validation = env::var("SAH_SHELL_SECURITY_ENABLE_VALIDATION").ok();
        let original_log_level = env::var("SAH_SHELL_AUDIT_LOG_LEVEL").ok();
        let original_max_size = env::var("SAH_SHELL_OUTPUT_MAX_SIZE").ok();

        // Ensure cleanup happens even if test panics
        struct EnvCleanup {
            validation: Option<String>,
            log_level: Option<String>,
            max_size: Option<String>,
        }
        impl Drop for EnvCleanup {
            fn drop(&mut self) {
                match &self.validation {
                    Some(val) => env::set_var("SAH_SHELL_SECURITY_ENABLE_VALIDATION", val),
                    None => env::remove_var("SAH_SHELL_SECURITY_ENABLE_VALIDATION"),
                }
                match &self.log_level {
                    Some(val) => env::set_var("SAH_SHELL_AUDIT_LOG_LEVEL", val),
                    None => env::remove_var("SAH_SHELL_AUDIT_LOG_LEVEL"),
                }
                match &self.max_size {
                    Some(val) => env::set_var("SAH_SHELL_OUTPUT_MAX_SIZE", val),
                    None => env::remove_var("SAH_SHELL_OUTPUT_MAX_SIZE"),
                }
            }
        }
        let _cleanup = EnvCleanup {
            validation: original_validation,
            log_level: original_log_level,
            max_size: original_max_size,
        };

        // Clean up any existing environment variables first
        env::remove_var("SAH_SHELL_SECURITY_ENABLE_VALIDATION");
        env::remove_var("SAH_SHELL_AUDIT_LOG_LEVEL");
        env::remove_var("SAH_SHELL_OUTPUT_MAX_OUTPUT_SIZE");

        // Test invalid boolean value
        env::set_var("SAH_SHELL_SECURITY_ENABLE_VALIDATION", "maybe");
        let loader = ConfigurationLoader::new()?.with_validation(false);
        let result = loader.apply_shell_env_overrides(ShellToolConfig::default());
        assert!(result.is_err());

        // Clean up and test invalid log level
        env::remove_var("SAH_SHELL_SECURITY_ENABLE_VALIDATION");
        env::set_var("SAH_SHELL_AUDIT_LOG_LEVEL", "invalid_level");
        let result = loader.apply_shell_env_overrides(ShellToolConfig::default());
        assert!(result.is_err());

        // Clean up and test invalid size format
        env::remove_var("SAH_SHELL_AUDIT_LOG_LEVEL");
        env::set_var("SAH_SHELL_OUTPUT_MAX_SIZE", "invalid_size");
        let result = loader.apply_shell_env_overrides(ShellToolConfig::default());
        assert!(result.is_err());

        // Cleanup will happen automatically via Drop

        Ok(())
    }
}
