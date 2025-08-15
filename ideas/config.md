# SwissArmyHammer Configuration System using Figment

## Overview

This specification outlines the implementation of a comprehensive configuration system for SwissArmyHammer using the `figment` crate. The system will support multiple configuration sources with a clear precedence order, multiple file formats, and environment variable integration.

## Current State

Currently, SwissArmyHammer has:
- Custom TOML configuration parsing in `src/sah_config/`
- Limited configuration file discovery
- Basic environment variable support
- Hardcoded configuration paths and formats

## Proposed Design

### 1. Configuration Precedence Order

Configuration sources should be merged in the following order (later sources override earlier ones):

1. **Default values** (hardcoded in application)
2. **Global config file** (`~/.swissarmyhammer/` directory)
3. **Project config file** (`.swissarmyhammer/` directory in current project)
4. **Local config file** (current working directory)
5. **Environment variables** (with `SAH_` or `SWISSARMYHAMMER_` prefix)
6. **Command line arguments** (highest priority)

### 2. Configuration File Discovery

#### File Names
Support both short and long form names:
- `sah.{toml,yaml,yml,json}`
- `swissarmyhammer.{toml,yaml,yml,json}`

#### Search Locations
1. **Current Working Directory (pwd)**
   ```
   ./sah.toml
   ./sah.yaml
   ./sah.yml
   ./sah.json
   ./swissarmyhammer.toml
   ./swissarmyhammer.yaml
   ./swissarmyhammer.yml
   ./swissarmyhammer.json
   ```

2. **Project SwissArmyHammer Directory**
   ```
   ./.swissarmyhammer/sah.toml
   ./.swissarmyhammer/sah.yaml
   ./.swissarmyhammer/sah.yml
   ./.swissarmyhammer/sah.json
   ./.swissarmyhammer/swissarmyhammer.toml
   ./.swissarmyhammer/swissarmyhammer.yaml
   ./.swissarmyhammer/swissarmyhammer.yml
   ./.swissarmyhammer/swissarmyhammer.json
   ```

3. **User Home SwissArmyHammer Directory**
   ```
   ~/.swissarmyhammer/sah.toml
   ~/.swissarmyhammer/sah.yaml
   ~/.swissarmyhammer/sah.yml
   ~/.swissarmyhammer/sah.json
   ~/.swissarmyhammer/swissarmyhammer.toml
   ~/.swissarmyhammer/swissarmyhammer.yaml
   ~/.swissarmyhammer/swissarmyhammer.yml
   ~/.swissarmyhammer/swissarmyhammer.json
   ```

### 3. Configuration Schema

#### 3.1 Core Configuration Structure
```toml
# General settings
debug = false
verbose = false
color = "auto"  # "auto", "always", "never"

[paths]
# Directory paths
home_dir = "~/.swissarmyhammer"
project_dir = ".swissarmyhammer"
cache_dir = "~/.cache/swissarmyhammer"
temp_dir = "/tmp/swissarmyhammer"

# Resource paths
prompts_dir = "prompts"
workflows_dir = "workflows"
issues_dir = "issues"
memoranda_dir = "memoranda"

[search]
# Semantic search configuration
enabled = true
model = "nomic-embed-text-v1.5"
cache_dir = "/tmp/.cache/fastembed"
batch_size = 32
max_text_length = 8000

[workflow]
# Workflow execution settings
cache_dir = "~/.swissarmyhammer/workflow_cache"
parallel_execution = true
max_concurrent_actions = 4
timeout_seconds = 300

[git]
# Git integration settings
auto_commit = false
commit_template = "SwissArmyHammer: {action}"
default_branch = "main"

[mcp]
# MCP server settings
enabled = true
port = 3000
log_level = "info"

[prompts]
# Prompt system settings
template_engine = "liquid"
strict_variables = false
auto_escape = true

[security]
# Security settings
allow_shell_commands = true
allowed_commands = []
blocked_commands = ["rm -rf", "format", "del /f"]
sandbox_mode = false
```

#### 3.2 Environment Variable Mapping
```bash
# Core settings
SAH_DEBUG=true
SAH_VERBOSE=true
SAH_COLOR=always

# Paths
SAH_HOME_DIR=/custom/sah/home
SAH_CACHE_DIR=/tmp/my-sah-cache
SWISSARMYHAMMER_PROMPTS_DIR=./my-prompts

# Search settings
SAH_SEARCH_ENABLED=false
SAH_SEARCH_MODEL=custom-model
SAH_SEARCH_CACHE_DIR=/custom/cache

# Workflow settings
SAH_WORKFLOW_PARALLEL_EXECUTION=false
SAH_WORKFLOW_MAX_CONCURRENT_ACTIONS=8

# Git settings
SAH_GIT_AUTO_COMMIT=true
SAH_GIT_DEFAULT_BRANCH=develop

# MCP settings
SAH_MCP_ENABLED=false
SAH_MCP_PORT=4000

# Security settings
SAH_SECURITY_SANDBOX_MODE=true
```

### 4. Implementation Details

#### 4.1 Figment Integration
```rust
use figment::{Figment, providers::{Format, Toml, Yaml, Json, Env}};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct SwissArmyHammerConfig {
    pub debug: bool,
    pub verbose: bool,
    pub color: ColorMode,
    pub paths: PathsConfig,
    pub search: SearchConfig,
    pub workflow: WorkflowConfig,
    pub git: GitConfig,
    pub mcp: McpConfig,
    pub prompts: PromptsConfig,
    pub security: SecurityConfig,
}

impl SwissArmyHammerConfig {
    pub fn load() -> Result<Self, ConfigError> {
        let mut figment = Figment::new()
            // Default configuration
            .merge(Toml::string(include_str!("default.toml")))
            
            // User config (~/.swissarmyhammer/)
            .merge(Toml::file("~/.swissarmyhammer/sah.toml"))
            .merge(Yaml::file("~/.swissarmyhammer/sah.yaml"))
            .merge(Json::file("~/.swissarmyhammer/sah.json"))
            .merge(Toml::file("~/.swissarmyhammer/swissarmyhammer.toml"))
            .merge(Yaml::file("~/.swissarmyhammer/swissarmyhammer.yaml"))
            .merge(Json::file("~/.swissarmyhammer/swissarmyhammer.json"))
            
            // Project config (./.swissarmyhammer/)  
            .merge(Toml::file(".swissarmyhammer/sah.toml"))
            .merge(Yaml::file(".swissarmyhammer/sah.yaml"))
            .merge(Json::file(".swissarmyhammer/sah.json"))
            .merge(Toml::file(".swissarmyhammer/swissarmyhammer.toml"))
            .merge(Yaml::file(".swissarmyhammer/swissarmyhammer.yaml"))
            .merge(Json::file(".swissarmyhammer/swissarmyhammer.json"))
            
            // Local config (pwd)
            .merge(Toml::file("sah.toml"))
            .merge(Yaml::file("sah.yaml"))
            .merge(Json::file("sah.json"))
            .merge(Toml::file("swissarmyhammer.toml"))
            .merge(Yaml::file("swissarmyhammer.yaml"))
            .merge(Json::file("swissarmyhammer.json"))
            
            // Environment variables
            .merge(Env::prefixed("SAH_"))
            .merge(Env::prefixed("SWISSARMYHAMMER_"));
            
        figment.extract()
    }
}
```

#### 4.2 Configuration Validation
```rust
impl SwissArmyHammerConfig {
    pub fn validate(&self) -> Result<(), Vec<ConfigError>> {
        let mut errors = Vec::new();
        
        // Validate paths exist and are accessible
        if !self.paths.home_dir.exists() {
            errors.push(ConfigError::InvalidPath("home_dir".into()));
        }
        
        // Validate search model is supported
        if !SUPPORTED_MODELS.contains(&self.search.model.as_str()) {
            errors.push(ConfigError::UnsupportedModel(self.search.model.clone()));
        }
        
        // Validate security settings
        if self.security.sandbox_mode && self.security.allow_shell_commands {
            errors.push(ConfigError::ConflictingSettings(
                "Cannot allow shell commands in sandbox mode".into()
            ));
        }
        
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }
}
```

### 5. Configuration Management Commands

#### 5.1 CLI Configuration Commands
```bash
# View current configuration
sah config show

# View configuration with sources
sah config show --sources

# Get specific configuration value
sah config get search.model
sah config get paths.cache_dir

# Set configuration value (writes to appropriate config file)
sah config set search.model "custom-model"
sah config set debug true

# Validate configuration
sah config validate

# Show configuration file locations
sah config locations

# Generate sample configuration file
sah config init --format toml --location local
sah config init --format yaml --location project
sah config init --format json --location global
```

#### 5.2 Configuration Profiles
```bash
# Use specific configuration profile
sah --profile development workflow run test
sah --profile production search index

# Profile-specific config files
sah.development.toml
sah.production.yaml
swissarmyhammer.staging.json
```

### 6. Migration Strategy

#### 6.1 From Current System
- Migrate existing `src/sah_config/` to use Figment
- Convert current TOML parsing to Figment-based approach
- Maintain backward compatibility with existing config files
- Provide migration tool for old configuration format

#### 6.2 Gradual Rollout
1. **Phase 1**: Implement Figment-based configuration loading
2. **Phase 2**: Add environment variable support
3. **Phase 3**: Implement multi-format file support  
4. **Phase 4**: Add configuration management CLI commands
5. **Phase 5**: Deprecate old configuration system

### 7. Error Handling

#### 7.1 Configuration Error Types
```rust
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Configuration file not found: {0}")]
    FileNotFound(String),
    
    #[error("Invalid configuration format: {0}")]
    InvalidFormat(String),
    
    #[error("Missing required configuration: {0}")]
    MissingRequired(String),
    
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    
    #[error("Unsupported model: {0}")]
    UnsupportedModel(String),
    
    #[error("Conflicting settings: {0}")]
    ConflictingSettings(String),
}
```

#### 7.2 Graceful Degradation
- Continue with defaults if config files are missing
- Warn about invalid configuration values
- Provide helpful error messages with suggestions
- Support partial configuration loading

### 8. Testing Strategy

#### 8.1 Unit Tests
- Configuration loading from different sources
- Precedence order validation
- Environment variable parsing
- Configuration validation logic

#### 8.2 Integration Tests
- Multi-format configuration file support
- Configuration command CLI interface
- Migration from old to new configuration system
- Cross-platform path handling

#### 8.3 Configuration Test Fixtures
```
tests/
├── configs/
│   ├── valid/
│   │   ├── sah.toml
│   │   ├── sah.yaml
│   │   ├── sah.json
│   │   └── swissarmyhammer.toml
│   ├── invalid/
│   │   ├── malformed.toml
│   │   ├── missing-required.yaml
│   │   └── conflicting.json
│   └── migration/
│       ├── old-format.toml
│       └── expected-new.toml
```

### 9. Documentation Updates

#### 9.1 User Documentation
- Configuration file format examples
- Environment variable reference
- Configuration precedence explanation
- Common configuration patterns

#### 9.2 Developer Documentation
- Figment integration implementation
- Configuration schema definition
- Error handling patterns
- Testing configuration loading

### 10. Performance Considerations

#### 10.1 Configuration Caching
- Cache loaded configuration to avoid repeated file I/O
- Implement configuration change detection
- Provide configuration reload mechanisms

#### 10.2 Lazy Loading
- Load configuration sections on demand
- Avoid expensive validation until needed
- Support partial configuration updates

### 11. Success Criteria

- [ ] Figment-based configuration system implemented
- [ ] Support for TOML, YAML, and JSON formats
- [ ] Proper precedence order (pwd → .swissarmyhammer/ → ~/.swissarmyhammer/)
- [ ] Environment variable integration with SAH_ and SWISSARMYHAMMER_ prefixes
- [ ] Both `sah.*` and `swissarmyhammer.*` filename support
- [ ] Configuration management CLI commands
- [ ] Migration path from existing configuration system
- [ ] Comprehensive error handling and validation
- [ ] Complete test coverage
- [ ] Updated documentation with examples

## Conclusion

This specification provides a robust, flexible configuration system using Figment that supports multiple formats, clear precedence rules, and comprehensive environment variable integration. The implementation will significantly improve the user experience while maintaining backward compatibility with existing configurations.