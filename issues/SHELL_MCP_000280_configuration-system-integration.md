# Configuration System Integration

Refer to /Users/wballard/github/sah-shell/ideas/shell.md

## Overview

Integrate the shell tool with the existing SwissArmyHammer configuration system, providing comprehensive configuration options for security, performance, and operational settings.

## Objective

Create a complete configuration framework for the shell tool that integrates with the existing `sah_config` system, supports environment-specific settings, and provides runtime configuration management.

## Requirements

### Configuration Schema Integration
- Define complete shell tool configuration schema
- Integrate with existing `sah_config` system
- Support global and per-execution settings
- Provide sensible defaults for all environments

### Environment-Specific Configuration
- Support development, testing, and production configurations
- Enable environment variable overrides
- Provide configuration validation and error handling
- Support runtime configuration updates

### Security Policy Configuration
- Configure command validation and restrictions
- Set directory access controls
- Define audit logging requirements
- Manage security policy inheritance

### Performance and Resource Configuration
- Configure timeout limits and defaults
- Set output size limits and truncation behavior
- Define resource usage constraints
- Enable performance monitoring settings

## Implementation Details

### Configuration Schema Definition
```rust
// In swissarmyhammer/src/sah_config/types.rs

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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ShellOutputConfig {
    /// Maximum output size before truncation
    pub max_output_size: String, // e.g., "10MB"
    
    /// Maximum line length before truncation
    pub max_line_length: usize,
    
    /// Enable binary content detection
    pub detect_binary_content: bool,
    
    /// Truncation strategy
    pub truncation_strategy: TruncationStrategy,
}

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
```

### Configuration Defaults
```rust
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
```

### Configuration Loading and Validation
```rust
// In swissarmyhammer/src/sah_config/loader.rs

impl ConfigLoader {
    pub fn load_shell_config(&self) -> Result<ShellToolConfig, ConfigError> {
        // Load from various sources with precedence
        let mut config = ShellToolConfig::default();
        
        // Load from configuration files
        if let Ok(file_config) = self.load_from_file("shell") {
            config = self.merge_shell_config(config, file_config)?;
        }
        
        // Override with environment variables
        config = self.apply_env_overrides(config)?;
        
        // Validate configuration
        self.validate_shell_config(&config)?;
        
        Ok(config)
    }
    
    fn validate_shell_config(&self, config: &ShellToolConfig) -> Result<(), ConfigError> {
        // Validate timeout ranges
        if config.execution.default_timeout < config.execution.min_timeout {
            return Err(ConfigError::InvalidValue {
                key: "execution.default_timeout".to_string(),
                value: config.execution.default_timeout.to_string(),
                reason: "Default timeout cannot be less than minimum timeout".to_string(),
            });
        }
        
        if config.execution.default_timeout > config.execution.max_timeout {
            return Err(ConfigError::InvalidValue {
                key: "execution.default_timeout".to_string(),
                value: config.execution.default_timeout.to_string(),
                reason: "Default timeout cannot exceed maximum timeout".to_string(),
            });
        }
        
        // Validate output size format
        parse_size_string(&config.output.max_output_size)
            .map_err(|_| ConfigError::InvalidValue {
                key: "output.max_output_size".to_string(),
                value: config.output.max_output_size.clone(),
                reason: "Invalid size format (e.g., '10MB', '1GB')".to_string(),
            })?;
        
        Ok(())
    }
}
```

### Environment Variable Support
```rust
impl ConfigLoader {
    fn apply_env_overrides(
        &self, 
        mut config: ShellToolConfig
    ) -> Result<ShellToolConfig, ConfigError> {
        // Security overrides
        if let Ok(val) = env::var("SAH_SHELL_SECURITY_ENABLE_VALIDATION") {
            config.security.enable_validation = val.parse()
                .map_err(|_| ConfigError::invalid_env_var("SAH_SHELL_SECURITY_ENABLE_VALIDATION", &val))?;
        }
        
        if let Ok(val) = env::var("SAH_SHELL_EXECUTION_DEFAULT_TIMEOUT") {
            config.execution.default_timeout = val.parse()
                .map_err(|_| ConfigError::invalid_env_var("SAH_SHELL_EXECUTION_DEFAULT_TIMEOUT", &val))?;
        }
        
        if let Ok(val) = env::var("SAH_SHELL_OUTPUT_MAX_SIZE") {
            config.output.max_output_size = val;
        }
        
        // Add more environment variable overrides as needed
        
        Ok(config)
    }
}
```

### Runtime Configuration Access
```rust
// In shell tool implementation
pub struct ShellToolContext {
    config: ShellToolConfig,
    // ... other fields
}

impl ShellToolContext {
    pub async fn new() -> Result<Self, ShellError> {
        let config_loader = ConfigLoader::new()?;
        let config = config_loader.load_shell_config()
            .map_err(ShellError::ConfigurationError)?;
        
        Ok(Self {
            config,
        })
    }
    
    pub fn security_policy(&self) -> &ShellSecurityConfig {
        &self.config.security
    }
    
    pub fn output_limits(&self) -> OutputLimits {
        OutputLimits {
            max_output_size: parse_size_string(&self.config.output.max_output_size).unwrap(),
            max_line_length: self.config.output.max_line_length,
            truncation_strategy: self.config.output.truncation_strategy.clone(),
        }
    }
}
```

### Configuration File Examples

#### Development Configuration (sah.toml)
```toml
[shell.security]
enable_validation = false  # Relaxed for development
blocked_commands = ["rm -rf /"]
max_command_length = 2000

[shell.output]
max_output_size = "50MB"  # Larger for development logs
max_line_length = 5000

[shell.execution]
default_timeout = 600  # 10 minutes for builds
max_timeout = 3600     # 1 hour maximum

[shell.audit]
enable_audit_logging = true
log_command_output = false  # Don't log output in dev
```

#### Production Configuration
```toml
[shell.security]
enable_validation = true
blocked_commands = ["rm", "format", "dd", "fdisk", "mkfs", "sudo"]
allowed_directories = ["/app", "/tmp/app"]
max_command_length = 500

[shell.output]
max_output_size = "5MB"   # Smaller for production
max_line_length = 1000

[shell.execution]
default_timeout = 300  # 5 minutes
max_timeout = 1800     # 30 minutes maximum

[shell.audit]
enable_audit_logging = true
log_command_output = true
max_audit_entry_size = 10000
```

## Integration Points

### Existing Configuration System
- Leverage existing `sah_config` infrastructure
- Use established configuration loading patterns
- Follow existing validation and error handling
- Maintain consistency with other tool configurations

### Template Integration
- Support configuration value templating
- Enable dynamic configuration based on context
- Allow configuration inheritance and overrides
- Support conditional configuration loading

## Acceptance Criteria

- [ ] Shell tool configuration schema defined and implemented
- [ ] Integration with existing `sah_config` system working
- [ ] Environment variable overrides functional
- [ ] Configuration validation comprehensive
- [ ] Default configurations appropriate for different environments
- [ ] Runtime configuration access working
- [ ] Configuration file examples documented
- [ ] Error handling for configuration issues clear

## Testing Requirements

- [ ] Configuration loading and validation tests
- [ ] Environment variable override tests
- [ ] Configuration merge and precedence tests
- [ ] Invalid configuration handling tests
- [ ] Runtime configuration access tests
- [ ] Cross-platform configuration file tests

## Documentation Requirements

- [ ] Configuration schema documentation
- [ ] Environment variable reference
- [ ] Configuration file examples for different environments
- [ ] Migration guide for existing configurations
- [ ] Troubleshooting guide for configuration issues

## Notes

- Configuration should be comprehensive but not overwhelming
- Default values should work well for most use cases
- Environment-specific configurations are critical for deployment
- Security configurations should err on the side of safety
- Performance configurations should balance usability with resource protection