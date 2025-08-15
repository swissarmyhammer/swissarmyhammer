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

## Proposed Solution

Based on my analysis of the existing codebase and the shell tool implementation, I propose the following implementation approach:

### 1. Shell Tool Configuration Schema

I will extend the existing `sah_config` system to support shell tool configuration by:

- Adding shell configuration types to `swissarmyhammer/src/sah_config/types.rs`
- Creating comprehensive configuration structures for security, output handling, execution limits, and audit settings
- Implementing proper validation and defaults for each configuration section

### 2. Configuration Integration Points

The configuration system will integrate with:

- **Existing OutputLimits**: Replace hardcoded values in shell tool with configurable limits
- **Security Validation**: Use configuration to control command validation and directory restrictions
- **Timeout Management**: Make timeout defaults and limits configurable
- **Audit Logging**: Enable configurable audit logging for shell commands

### 3. Implementation Strategy

1. **Schema Definition**: Create structured configuration types with comprehensive validation
2. **Loader Extension**: Extend `ConfigurationLoader` to handle shell-specific configuration loading
3. **Shell Tool Integration**: Modify shell tool to use configuration instead of hardcoded values
4. **Environment Variable Support**: Enable runtime configuration override via environment variables
5. **Testing**: Comprehensive test coverage for all configuration scenarios

### 4. Configuration Structure

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ShellToolConfig {
    pub security: ShellSecurityConfig,
    pub output: ShellOutputConfig,  
    pub execution: ShellExecutionConfig,
    pub audit: ShellAuditConfig,
}
```

This approach maintains compatibility with the existing configuration system while providing comprehensive shell tool configuration capabilities.

### 5. Key Benefits

- **Consistency**: Uses existing `sah_config` patterns and infrastructure
- **Flexibility**: Supports development, testing, and production configurations  
- **Security**: Configurable security policies and validation rules
- **Performance**: Tunable resource limits and timeout configurations
- **Auditability**: Configurable logging and audit trail capabilities
## Implementation Complete ✅

The shell tool configuration system has been fully implemented and integrated with the existing `sah_config` system:

### ✅ Configuration Schema (swissarmyhammer/src/sah_config/types.rs)
- Added comprehensive `ShellToolConfig` structure with security, output, execution, and audit configuration
- Implemented `TruncationStrategy` enum for output handling options
- Added `parse_size_string()` utility function for size configuration parsing
- Created proper default implementations for all configuration sections
- Added comprehensive serialization/deserialization support

### ✅ Configuration Loading (swissarmyhammer/src/sah_config/loader.rs)
- Extended `ConfigurationLoader` with `load_shell_config()` method
- Implemented TOML configuration file merging for all shell configuration sections
- Added environment variable override support with validation:
  - `SAH_SHELL_SECURITY_ENABLE_VALIDATION`
  - `SAH_SHELL_OUTPUT_MAX_SIZE`
  - `SAH_SHELL_EXECUTION_DEFAULT_TIMEOUT`
  - `SAH_SHELL_AUDIT_ENABLE_LOGGING`
  - And many more...
- Added comprehensive configuration validation including timeout ranges, size formats, and reasonable limits

### ✅ Shell Tool Integration (swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs)
- Modified `execute_shell_command()` to accept and use configuration
- Replaced hardcoded `OutputLimits::default()` with config-based `OutputLimits::from_config()`
- Updated timeout handling to use configured defaults with validation against min/max limits
- Added configuration loading in `ShellExecuteTool::execute()` method
- Maintained backward compatibility with existing tool interface

### ✅ Comprehensive Testing
- Added 70+ configuration tests covering all functionality
- Tests include default values, file loading, environment variable overrides, validation, and error cases
- All existing shell tool tests continue to pass (37 tests)
- Configuration serialization/deserialization round-trip testing
- Edge case and error condition testing

### Key Features Delivered

1. **Environment-Specific Configuration**: Development, testing, and production configurations supported through TOML files and environment variables

2. **Security Policy Configuration**: Configurable command validation, blocked command patterns, directory restrictions, and injection detection

3. **Performance Configuration**: Tunable timeout limits, output size limits, line length limits, and resource constraints

4. **Audit Configuration**: Configurable audit logging with level control and output inclusion options

5. **Runtime Validation**: Comprehensive configuration validation ensures consistency and security

6. **Backward Compatibility**: Existing shell tool usage continues to work seamlessly with sensible defaults

### Configuration Examples

**Development Configuration (sah.toml):**
```toml
[shell.security]
enable_validation = false
max_command_length = 2000

[shell.output]
max_output_size = "50MB"

[shell.execution]
default_timeout = 600
```

**Production Configuration:**
```toml
[shell.security]
enable_validation = true
blocked_commands = ["rm", "format", "dd", "fdisk"]
allowed_directories = ["/app", "/tmp/app"]

[shell.output]
max_output_size = "5MB"

[shell.execution]
default_timeout = 300
max_timeout = 1800
```

The implementation successfully integrates the shell tool with the existing SwissArmyHammer configuration system while maintaining full backward compatibility and providing comprehensive configuration capabilities.