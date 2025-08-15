use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

/// Represents a configuration value from sah.toml
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConfigValue {
    /// String value from configuration
    String(String),
    /// Integer value from configuration
    Integer(i64),
    /// Floating point value from configuration
    Float(f64),
    /// Boolean value from configuration
    Boolean(bool),
    /// Array of configuration values
    Array(Vec<ConfigValue>),
    /// Table (map) of configuration values
    Table(HashMap<String, ConfigValue>),
}

impl ConfigValue {
    /// Convert ConfigValue to liquid::model::Value for template rendering
    pub fn to_liquid_value(&self) -> liquid::model::Value {
        match self {
            ConfigValue::String(s) => liquid::model::Value::scalar(s.clone()),
            ConfigValue::Integer(i) => liquid::model::Value::scalar(*i),
            ConfigValue::Float(f) => liquid::model::Value::scalar(*f),
            ConfigValue::Boolean(b) => liquid::model::Value::scalar(*b),
            ConfigValue::Array(arr) => {
                let liquid_array: Vec<liquid::model::Value> =
                    arr.iter().map(|v| v.to_liquid_value()).collect();
                liquid::model::Value::Array(liquid_array)
            }
            ConfigValue::Table(table) => {
                let mut liquid_object = liquid::model::Object::new();
                for (key, value) in table {
                    liquid_object.insert(key.clone().into(), value.to_liquid_value());
                }
                liquid::model::Value::Object(liquid_object)
            }
        }
    }

    /// Get the type name as a string for error messages
    pub fn type_name(&self) -> &'static str {
        match self {
            ConfigValue::String(_) => "string",
            ConfigValue::Integer(_) => "integer",
            ConfigValue::Float(_) => "float",
            ConfigValue::Boolean(_) => "boolean",
            ConfigValue::Array(_) => "array",
            ConfigValue::Table(_) => "table",
        }
    }
}

/// Cache metadata for configuration files
#[derive(Debug, Clone)]
pub struct CacheMetadata {
    /// Last modification time of the source file
    pub last_modified: SystemTime,
    /// Size of the source file in bytes
    pub file_size: u64,
    /// Time when the configuration was loaded into cache
    pub loaded_at: SystemTime,
}

impl CacheMetadata {
    /// Create new cache metadata from file system metadata
    pub fn from_file(path: &std::path::Path) -> Result<Self, std::io::Error> {
        let metadata = std::fs::metadata(path)?;
        Ok(Self {
            last_modified: metadata.modified()?,
            file_size: metadata.len(),
            loaded_at: SystemTime::now(),
        })
    }

    /// Check if the cache is still valid by comparing with current file metadata
    pub fn is_valid(&self, path: &std::path::Path) -> bool {
        match std::fs::metadata(path) {
            Ok(current_metadata) => {
                // Check if file size changed
                if current_metadata.len() != self.file_size {
                    return false;
                }

                // Check if modification time changed
                match current_metadata.modified() {
                    Ok(current_modified) => current_modified <= self.last_modified,
                    Err(_) => false, // If we can't get modification time, assume invalid
                }
            }
            Err(_) => false, // If file doesn't exist or can't be accessed, cache is invalid
        }
    }

    /// Check if the cache has expired based on a TTL (time-to-live)
    pub fn is_expired(&self, ttl_seconds: u64) -> bool {
        match SystemTime::now().duration_since(self.loaded_at) {
            Ok(age) => age.as_secs() > ttl_seconds,
            Err(_) => true, // If time calculation fails, assume expired
        }
    }
}

/// Main configuration structure containing all sah.toml variables
#[derive(Debug, Clone)]
pub struct Configuration {
    /// The parsed configuration values
    values: HashMap<String, ConfigValue>,
    /// Path to the configuration file (if loaded from file)
    file_path: Option<PathBuf>,
    /// Cache metadata for tracking file changes
    cache_metadata: Option<CacheMetadata>,
}

impl Configuration {
    /// Create a new empty configuration
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
            file_path: None,
            cache_metadata: None,
        }
    }

    /// Create a configuration with values and file path
    pub fn with_values(values: HashMap<String, ConfigValue>, file_path: Option<PathBuf>) -> Self {
        Self {
            values,
            file_path,
            cache_metadata: None,
        }
    }

    /// Create a configuration with values, file path, and cache metadata
    pub fn with_cache_metadata(
        values: HashMap<String, ConfigValue>,
        file_path: Option<PathBuf>,
        cache_metadata: Option<CacheMetadata>,
    ) -> Self {
        Self {
            values,
            file_path,
            cache_metadata,
        }
    }

    /// Get a configuration value by key
    pub fn get(&self, key: &str) -> Option<&ConfigValue> {
        self.values.get(key)
    }

    /// Get all configuration values
    pub fn values(&self) -> &HashMap<String, ConfigValue> {
        &self.values
    }

    /// Get the file path if this configuration was loaded from a file
    pub fn file_path(&self) -> Option<&PathBuf> {
        self.file_path.as_ref()
    }

    /// Insert a new configuration value
    pub fn insert(&mut self, key: String, value: ConfigValue) {
        self.values.insert(key, value);
    }

    /// Check if the configuration is empty
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Get the number of configuration values
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Convert all configuration values to liquid objects for template rendering
    pub fn to_liquid_object(&self) -> liquid::model::Object {
        let mut object = liquid::model::Object::new();
        for (key, value) in &self.values {
            object.insert(key.clone().into(), value.to_liquid_value());
        }
        object
    }

    /// Get cache metadata if available
    pub fn cache_metadata(&self) -> Option<&CacheMetadata> {
        self.cache_metadata.as_ref()
    }

    /// Set cache metadata
    pub fn set_cache_metadata(&mut self, metadata: CacheMetadata) {
        self.cache_metadata = Some(metadata);
    }

    /// Check if the cached configuration is still valid
    ///
    /// Returns `true` if:
    /// - No cache metadata exists (not cached)
    /// - Cache metadata exists and file hasn't changed
    /// - No file path exists (in-memory configuration)
    pub fn is_cache_valid(&self) -> bool {
        match (&self.cache_metadata, &self.file_path) {
            (Some(metadata), Some(path)) => metadata.is_valid(path),
            (None, _) => true, // No cache metadata, assume valid
            (_, None) => true, // No file path, in-memory config is always valid
        }
    }

    /// Check if the cached configuration has expired
    ///
    /// # Arguments
    /// * `ttl_seconds` - Time-to-live in seconds
    ///
    /// Returns `true` if cache has expired, `false` if still valid or no cache exists
    pub fn is_cache_expired(&self, ttl_seconds: u64) -> bool {
        match &self.cache_metadata {
            Some(metadata) => metadata.is_expired(ttl_seconds),
            None => false, // No cache, so not expired
        }
    }

    /// Check if configuration should be reloaded
    ///
    /// Returns `true` if either cache is invalid or expired
    pub fn should_reload(&self, ttl_seconds: u64) -> bool {
        !self.is_cache_valid() || self.is_cache_expired(ttl_seconds)
    }

    /// Get cache age in seconds since loading
    pub fn cache_age_seconds(&self) -> Option<u64> {
        self.cache_metadata.as_ref().and_then(|metadata| {
            SystemTime::now()
                .duration_since(metadata.loaded_at)
                .ok()
                .map(|d| d.as_secs())
        })
    }
}

impl Default for Configuration {
    fn default() -> Self {
        Self::new()
    }
}

/// Shell tool configuration structure
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ShellToolConfig {
    /// Security settings for command execution
    pub security: ShellSecurityConfig,
    
    /// Output handling and limits
    pub output: ShellOutputConfig,
    
    /// Timeout and execution limits
    pub execution: ShellExecutionConfig,
    
    /// Audit and logging configuration
    pub audit: ShellAuditConfig,
}

/// Security configuration for shell command execution
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ShellSecurityConfig {
    /// Enable command validation and security checks
    pub enable_validation: bool,
    
    /// List of blocked command patterns
    pub blocked_commands: Vec<String>,
    
    /// Allowed directories for command execution
    pub allowed_directories: Option<Vec<String>>,
    
    /// Maximum allowed command length
    pub max_command_length: usize,
    
    /// Enable injection pattern detection
    pub enable_injection_detection: bool,
}

/// Output handling configuration for shell commands
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ShellOutputConfig {
    /// Maximum output size before truncation (e.g., "10MB")
    pub max_output_size: String,
    
    /// Maximum line length before truncation
    pub max_line_length: usize,
    
    /// Enable binary content detection
    pub detect_binary_content: bool,
    
    /// Truncation strategy
    pub truncation_strategy: TruncationStrategy,
}

/// Execution limits configuration for shell commands
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ShellExecutionConfig {
    /// Default timeout for commands (seconds)
    pub default_timeout: u64,
    
    /// Maximum allowed timeout (seconds)
    pub max_timeout: u64,
    
    /// Minimum allowed timeout (seconds)
    pub min_timeout: u64,
    
    /// Enable process tree cleanup
    pub cleanup_process_tree: bool,
}

/// Audit and logging configuration for shell commands
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ShellAuditConfig {
    /// Enable audit logging
    pub enable_audit_logging: bool,
    
    /// Audit log level
    pub log_level: String,
    
    /// Include command output in audit logs
    pub log_command_output: bool,
    
    /// Maximum audit log entry size
    pub max_audit_entry_size: usize,
}

/// Truncation strategy for output handling
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum TruncationStrategy {
    /// Preserve line structure when truncating
    PreserveStructure,
    /// Simple byte-based truncation
    SimpleTruncation,
    /// Truncate at word boundaries when possible
    WordBoundary,
}

impl Default for ShellToolConfig {
    fn default() -> Self {
        Self {
            security: ShellSecurityConfig::default(),
            output: ShellOutputConfig::default(),
            execution: ShellExecutionConfig::default(),
            audit: ShellAuditConfig::default(),
        }
    }
}

impl Default for ShellSecurityConfig {
    fn default() -> Self {
        Self {
            enable_validation: true,
            blocked_commands: vec![
                "rm -rf /".to_string(),
                "format".to_string(),
                "dd if=".to_string(),
                "mkfs".to_string(),
                "fdisk".to_string(),
            ],
            allowed_directories: None, // No restrictions by default
            max_command_length: 1000,
            enable_injection_detection: true,
        }
    }
}

impl Default for ShellOutputConfig {
    fn default() -> Self {
        Self {
            max_output_size: "10MB".to_string(),
            max_line_length: 2000,
            detect_binary_content: true,
            truncation_strategy: TruncationStrategy::PreserveStructure,
        }
    }
}

impl Default for ShellExecutionConfig {
    fn default() -> Self {
        Self {
            default_timeout: 300, // 5 minutes
            max_timeout: 1800,    // 30 minutes
            min_timeout: 1,       // 1 second
            cleanup_process_tree: true,
        }
    }
}

impl Default for ShellAuditConfig {
    fn default() -> Self {
        Self {
            enable_audit_logging: false, // Disabled by default to avoid noise
            log_level: "info".to_string(),
            log_command_output: false,   // Disabled by default for security
            max_audit_entry_size: 10000, // 10KB limit for audit entries
        }
    }
}

impl Default for TruncationStrategy {
    fn default() -> Self {
        Self::PreserveStructure
    }
}

/// Utility function to parse size strings (e.g., "10MB", "1GB") to bytes
pub fn parse_size_string(size_str: &str) -> Result<usize, String> {
    let size_str = size_str.trim().to_uppercase();
    
    if size_str.is_empty() {
        return Err("Empty size string".to_string());
    }
    
    // Extract numeric part and unit
    let (numeric_part, unit) = if let Some(pos) = size_str.find(|c: char| c.is_alphabetic()) {
        (&size_str[..pos], &size_str[pos..])
    } else {
        // No unit, assume bytes
        return size_str.parse::<usize>()
            .map_err(|_| format!("Invalid numeric value: {}", size_str));
    };
    
    let base_size: usize = numeric_part.parse()
        .map_err(|_| format!("Invalid numeric value: {}", numeric_part))?;
    
    let multiplier = match unit {
        "B" | "" => 1,
        "KB" => 1_024,
        "MB" => 1_024 * 1_024,
        "GB" => 1_024 * 1_024 * 1_024,
        _ => return Err(format!("Unknown size unit: {}", unit)),
    };
    
    base_size.checked_mul(multiplier)
        .ok_or_else(|| "Size value too large".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_value_to_liquid_value() {
        // Test string conversion
        let string_val = ConfigValue::String("test".to_string());
        assert_eq!(
            string_val.to_liquid_value(),
            liquid::model::Value::scalar("test")
        );

        // Test integer conversion
        let int_val = ConfigValue::Integer(42);
        assert_eq!(int_val.to_liquid_value(), liquid::model::Value::scalar(42));

        // Test boolean conversion
        let bool_val = ConfigValue::Boolean(true);
        assert_eq!(
            bool_val.to_liquid_value(),
            liquid::model::Value::scalar(true)
        );

        // Test array conversion
        let array_val = ConfigValue::Array(vec![
            ConfigValue::String("item1".to_string()),
            ConfigValue::Integer(2),
        ]);
        let liquid_array = array_val.to_liquid_value();
        assert!(matches!(liquid_array, liquid::model::Value::Array(_)));
    }

    #[test]
    fn test_configuration_operations() {
        let mut config = Configuration::new();
        assert!(config.is_empty());
        assert_eq!(config.len(), 0);

        config.insert(
            "key1".to_string(),
            ConfigValue::String("value1".to_string()),
        );
        assert!(!config.is_empty());
        assert_eq!(config.len(), 1);

        let value = config.get("key1");
        assert!(value.is_some());
        assert_eq!(value.unwrap(), &ConfigValue::String("value1".to_string()));
    }

    #[test]
    fn test_config_value_type_names() {
        assert_eq!(
            ConfigValue::String("test".to_string()).type_name(),
            "string"
        );
        assert_eq!(ConfigValue::Integer(42).type_name(), "integer");
        assert_eq!(ConfigValue::Float(3.15).type_name(), "float"); // Using 3.15 to avoid clippy PI warning
        assert_eq!(ConfigValue::Boolean(true).type_name(), "boolean");
        assert_eq!(ConfigValue::Array(vec![]).type_name(), "array");
        assert_eq!(ConfigValue::Table(HashMap::new()).type_name(), "table");
    }

    #[test]
    fn test_cache_metadata_creation() -> Result<(), Box<dyn std::error::Error>> {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create a temporary file
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "test = 'value'")?;

        // Create cache metadata from the file
        let metadata = CacheMetadata::from_file(temp_file.path())?;

        assert_eq!(metadata.file_size, 15); // Length of "test = 'value'\n"
        assert!(metadata.loaded_at <= SystemTime::now());

        Ok(())
    }

    #[test]
    fn test_cache_validity() -> Result<(), Box<dyn std::error::Error>> {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create a temporary file
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "test = 'value'")?;

        // Create cache metadata
        let metadata = CacheMetadata::from_file(temp_file.path())?;

        // Cache should be valid immediately
        assert!(metadata.is_valid(temp_file.path()));

        // Modify the file
        writeln!(temp_file, "additional = 'data'")?;

        // Cache should now be invalid
        assert!(!metadata.is_valid(temp_file.path()));

        Ok(())
    }

    #[test]
    fn test_cache_expiration() -> Result<(), Box<dyn std::error::Error>> {
        use std::io::Write;
        use std::time::{Duration, SystemTime};
        use tempfile::NamedTempFile;

        // Create a temporary file
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "test = 'value'")?;

        // Create cache metadata with a past time
        let past_time = SystemTime::now() - Duration::from_secs(10);
        let metadata = CacheMetadata {
            last_modified: SystemTime::now(),
            file_size: 15,
            loaded_at: past_time,
        };

        // Cache should not be expired with a large TTL
        assert!(!metadata.is_expired(3600)); // 1 hour

        // Cache should be expired with a TTL of 5 seconds (loaded 10 seconds ago)
        assert!(metadata.is_expired(5));

        Ok(())
    }

    #[test]
    fn test_configuration_caching() {
        let mut config = Configuration::new();

        // Initially no cache metadata
        assert!(config.cache_metadata().is_none());
        assert!(config.is_cache_valid()); // No cache = valid
        assert!(!config.is_cache_expired(3600)); // No cache = not expired

        // Create dummy cache metadata
        let cache_metadata = CacheMetadata {
            last_modified: SystemTime::now(),
            file_size: 100,
            loaded_at: SystemTime::now(),
        };

        config.set_cache_metadata(cache_metadata);
        assert!(config.cache_metadata().is_some());

        // Cache age should be 0 (just created)
        let age = config.cache_age_seconds().unwrap_or(999);
        assert!(age <= 1); // Should be 0 or 1 second
    }

    #[test]
    fn test_configuration_should_reload() {
        use std::time::{Duration, SystemTime};

        let mut config = Configuration::new();

        // No cache = should not reload
        assert!(!config.should_reload(300));

        // Add cache metadata but no file path
        let past_time = SystemTime::now() - Duration::from_secs(10);
        let cache_metadata = CacheMetadata {
            last_modified: SystemTime::now(),
            file_size: 100,
            loaded_at: past_time,
        };
        config.set_cache_metadata(cache_metadata);

        // Still should not reload (no file path, so cache validity doesn't matter)
        assert!(!config.should_reload(300));

        // Test with expired cache - should reload because TTL is less than age
        assert!(config.should_reload(5)); // TTL of 5 seconds but loaded 10 seconds ago
    }

    #[test]
    fn test_shell_tool_config_defaults() {
        let config = ShellToolConfig::default();
        
        // Test security defaults
        assert!(config.security.enable_validation);
        assert!(!config.security.blocked_commands.is_empty());
        assert_eq!(config.security.max_command_length, 1000);
        assert!(config.security.enable_injection_detection);
        assert!(config.security.allowed_directories.is_none());
        
        // Test output defaults
        assert_eq!(config.output.max_output_size, "10MB");
        assert_eq!(config.output.max_line_length, 2000);
        assert!(config.output.detect_binary_content);
        assert_eq!(config.output.truncation_strategy, TruncationStrategy::PreserveStructure);
        
        // Test execution defaults
        assert_eq!(config.execution.default_timeout, 300);
        assert_eq!(config.execution.max_timeout, 1800);
        assert_eq!(config.execution.min_timeout, 1);
        assert!(config.execution.cleanup_process_tree);
        
        // Test audit defaults
        assert!(!config.audit.enable_audit_logging); // Disabled by default
        assert_eq!(config.audit.log_level, "info");
        assert!(!config.audit.log_command_output); // Disabled by default for security
        assert_eq!(config.audit.max_audit_entry_size, 10000);
    }

    #[test]
    fn test_truncation_strategy_default() {
        let strategy = TruncationStrategy::default();
        assert_eq!(strategy, TruncationStrategy::PreserveStructure);
    }

    #[test]
    fn test_parse_size_string() -> Result<(), String> {
        // Test bytes
        assert_eq!(parse_size_string("1000")?, 1000);
        assert_eq!(parse_size_string("1000B")?, 1000);
        
        // Test kilobytes
        assert_eq!(parse_size_string("1KB")?, 1_024);
        assert_eq!(parse_size_string("2KB")?, 2 * 1_024);
        
        // Test megabytes
        assert_eq!(parse_size_string("1MB")?, 1_024 * 1_024);
        assert_eq!(parse_size_string("10MB")?, 10 * 1_024 * 1_024);
        
        // Test gigabytes
        assert_eq!(parse_size_string("1GB")?, 1_024 * 1_024 * 1_024);
        
        // Test with spaces
        assert_eq!(parse_size_string(" 1MB ")?, 1_024 * 1_024);
        
        // Test case insensitive
        assert_eq!(parse_size_string("1mb")?, 1_024 * 1_024);
        assert_eq!(parse_size_string("1Mb")?, 1_024 * 1_024);
        
        Ok(())
    }

    #[test]
    fn test_parse_size_string_errors() {
        // Test empty string
        assert!(parse_size_string("").is_err());
        assert!(parse_size_string("   ").is_err());
        
        // Test invalid numeric values
        assert!(parse_size_string("abc").is_err());
        assert!(parse_size_string("abc MB").is_err());
        assert!(parse_size_string("-1MB").is_err());
        
        // Test unknown units
        assert!(parse_size_string("1TB").is_err());
        assert!(parse_size_string("1XYZ").is_err());
    }

    #[test]
    fn test_shell_security_config_blocked_commands() {
        let config = ShellSecurityConfig::default();
        
        // Verify some dangerous commands are blocked by default
        assert!(config.blocked_commands.contains(&"rm -rf /".to_string()));
        assert!(config.blocked_commands.contains(&"format".to_string()));
        assert!(config.blocked_commands.contains(&"dd if=".to_string()));
        assert!(config.blocked_commands.contains(&"mkfs".to_string()));
        assert!(config.blocked_commands.contains(&"fdisk".to_string()));
    }

    #[test]
    fn test_shell_config_serialization() {
        let config = ShellToolConfig::default();
        
        // Test serialization to JSON
        let json = serde_json::to_string(&config).expect("Should serialize to JSON");
        assert!(!json.is_empty());
        
        // Test deserialization from JSON
        let deserialized: ShellToolConfig = serde_json::from_str(&json).expect("Should deserialize from JSON");
        
        // Compare some key fields to ensure round-trip works
        assert_eq!(config.security.enable_validation, deserialized.security.enable_validation);
        assert_eq!(config.output.max_output_size, deserialized.output.max_output_size);
        assert_eq!(config.execution.default_timeout, deserialized.execution.default_timeout);
        assert_eq!(config.audit.log_level, deserialized.audit.log_level);
    }

    #[test]
    fn test_truncation_strategy_serialization() {
        let strategies = vec![
            TruncationStrategy::PreserveStructure,
            TruncationStrategy::SimpleTruncation,
            TruncationStrategy::WordBoundary,
        ];
        
        for strategy in strategies {
            let json = serde_json::to_string(&strategy).expect("Should serialize");
            let deserialized: TruncationStrategy = serde_json::from_str(&json).expect("Should deserialize");
            assert_eq!(strategy, deserialized);
        }
    }
}
