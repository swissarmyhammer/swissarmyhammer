# SwissArmyHammer Configuration Guide

This guide provides comprehensive documentation for SwissArmyHammer's configuration system, which uses the Figment library to support multiple file formats, environment variables, and proper precedence handling.

## Table of Contents

- [Overview](#overview)
- [File Discovery](#file-discovery)
- [Supported Formats](#supported-formats)
- [Precedence Order](#precedence-order)
- [Environment Variables](#environment-variables)
- [Environment Variable Substitution](#environment-variable-substitution)
- [Template Integration](#template-integration)
- [Advanced Usage](#advanced-usage)
- [Model Configuration](#model-configuration)
- [Troubleshooting](#troubleshooting)

## Overview

SwissArmyHammer's configuration system is designed to be flexible and powerful while maintaining simplicity for basic use cases. Key features include:

- **Multiple formats**: TOML, YAML, and JSON support
- **Flexible discovery**: Automatic file discovery in standard locations
- **Environment integration**: Support for environment variables with substitution
- **Proper precedence**: Clear, predictable configuration merging
- **Template integration**: Configuration values automatically available in templates
- **No caching**: Fresh configuration loaded on each use (edit-friendly)

## File Discovery

### Supported File Names

SwissArmyHammer searches for configuration files using both short and descriptive names:

- **Short form**: `sah.{toml,yaml,yml,json}`
- **Long form**: `swissarmyhammer.{toml,yaml,yml,json}`

### Search Locations

Configuration files are discovered in the following directories (in precedence order):

1. **Global Configuration**: `~/.swissarmyhammer/`
   - User-wide settings
   - Shared across all projects
   - Good for personal preferences and credentials

2. **Project Configuration**: `./.swissarmyhammer/`
   - Project-specific settings
   - Version controlled with your project
   - Override global settings for specific projects

### Discovery Process

The system searches each location for configuration files in this order:

1. `sah.toml`
2. `sah.yaml`
3. `sah.yml`
4. `sah.json`
5. `swissarmyhammer.toml`
6. `swissarmyhammer.yaml`
7. `swissarmyhammer.yml`
8. `swissarmyhammer.json`

**Note**: All found files are merged according to precedence rules - later files override earlier ones.

## Supported Formats

### TOML Format

TOML (Tom's Obvious, Minimal Language) is the recommended format for its readability and robust typing.
### YAML Format

YAML provides excellent readability and supports complex nested structures.

### JSON Format

JSON provides universal compatibility and programmatic generation support.

## Precedence Order

Configuration sources are merged in this specific order, with later sources overriding earlier ones:

1. **Default Values** (lowest precedence)
   - Built-in application defaults
   - Hardcoded fallback values

2. **Global Config Files**
   - `~/.swissarmyhammer/sah.*`
   - User-wide preferences
   - Shared across all projects

3. **Project Config Files**
   - `./.swissarmyhammer/sah.*`
   - Project-specific overrides
   - Version controlled settings

4. **Environment Variables**
   - `SAH_*` and `SWISSARMYHAMMER_*` prefixed variables
   - Runtime configuration
   - Deployment-specific values

5. **CLI Arguments** (highest precedence)
   - Command-line overrides
   - One-time configuration changes

### Precedence Example

Given these configuration sources:

**Global config** (`~/.swissarmyhammer/sah.toml`):
```toml
[app]
name = "GlobalApp"
debug = true
version = "1.0.0"

[database]
host = "global-db"
port = 5432
```

**Project config** (`./.swissarmyhammer/sah.toml`):
```toml
[app]
name = "ProjectApp"  # Overrides global
# debug not specified - uses global value

[database]
host = "project-db"  # Overrides global
# port not specified - uses global value

[features]
new_feature = true   # New value, not in global
```

**Environment variables**:
```bash
export SAH_APP_DEBUG="false"  # Overrides both configs
export SAH_DATABASE_PORT="3306"  # Overrides global
```

**Final merged configuration**:
```toml
[app]
name = "ProjectApp"     # From project config
debug = false           # From environment variable
version = "1.0.0"       # From global config

[database]
host = "project-db"     # From project config
port = 3306             # From environment variable

[features]
new_feature = true      # From project config
```

## Environment Variables

### Variable Naming

SwissArmyHammer supports two environment variable prefixes:

- **`SAH_`**: Short, convenient prefix
- **`SWISSARMYHAMMER_`**: Explicit, descriptive prefix

Both prefixes support the same functionality and precedence rules.

### Nested Key Mapping

Environment variables use underscore separation to represent nested configuration keys:

| Environment Variable | Configuration Key |
|---------------------|-------------------|
| `SAH_APP_NAME` | `app.name` |
| `SAH_DATABASE_HOST` | `database.host` |
| `SAH_DATABASE_CREDENTIALS_USERNAME` | `database.credentials.username` |
| `SAH_FEATURES_EXPERIMENTAL_UI` | `features.experimental_ui` |

### Type Handling

Environment variables are automatically converted to appropriate types:

```bash
# String values (default)
export SAH_APP_NAME="MyProject"
export SAH_DATABASE_HOST="localhost"

# Boolean values
export SAH_DEBUG="true"
export SAH_FEATURES_TELEMETRY="false"

# Numeric values
export SAH_DATABASE_PORT="5432"
export SAH_APP_MAX_WORKERS="10"

# Array values (JSON format)
export SAH_FEATURES_ENABLED_MODULES='["auth", "api", "web"]'

# Object values (JSON format)
export SAH_DATABASE_CREDENTIALS='{"username": "admin", "database": "prod"}'
```

### Common Environment Variable Patterns

```bash
#!/bin/bash
# Development environment
export SAH_APP_DEBUG="true"
export SAH_APP_LOG_LEVEL="debug"
export SAH_DATABASE_HOST="localhost"
export SAH_DATABASE_PORT="5432"

# Production environment
export SAH_APP_DEBUG="false"
export SAH_APP_LOG_LEVEL="warn"
export SAH_DATABASE_HOST="prod-db.example.com"
export SAH_DATABASE_PORT="5432"
export SAH_DATABASE_SSL_ENABLED="true"

# Security credentials
export SAH_DATABASE_PASSWORD="${DATABASE_PASSWORD}"
export SAH_API_KEY="${API_SECRET_KEY}"
export SAH_JWT_SECRET="${JWT_SECRET}"
```

## Environment Variable Substitution

Configuration files support environment variable substitution using shell-style syntax.

### Basic Substitution

Replace placeholders with environment variable values:

```toml
# Basic substitution
database_url = "${DATABASE_URL}"
api_key = "${API_KEY}"
secret_key = "${JWT_SECRET}"

# In nested structures
[database]
host = "${DB_HOST}"
port = "${DB_PORT}"
password = "${DB_PASSWORD}"

[app]
name = "${APP_NAME}"
version = "${BUILD_VERSION}"
```

### Default Values

Provide fallback values when environment variables are not set:

```toml
# With default values
database_url = "${DATABASE_URL:-postgresql://localhost:5432/mydb}"
debug = "${DEBUG:-false}"
log_level = "${LOG_LEVEL:-info}"
max_connections = "${MAX_CONNECTIONS:-10}"

# Complex defaults
[app]
name = "${APP_NAME:-SwissArmyHammer}"
version = "${VERSION:-1.0.0}"
environment = "${ENVIRONMENT:-development}"

[database]
host = "${DB_HOST:-localhost}"
port = "${DB_PORT:-5432}"
ssl = "${DB_SSL:-false}"
timeout = "${DB_TIMEOUT:-30}"
```

### Advanced Substitution Patterns

```toml
# Conditional configuration based on environment
database_url = "${DATABASE_URL:-postgresql://${DB_USER:-admin}:${DB_PASS}@${DB_HOST:-localhost}:${DB_PORT:-5432}/${DB_NAME:-myapp}}"

# Environment-specific settings
[app]
debug = "${DEBUG:-false}"
log_level = "${LOG_LEVEL:-${DEBUG:+debug}:-info}"  # debug if DEBUG=true, otherwise info

# Feature flags from environment
[features]
experimental = "${ENABLE_EXPERIMENTAL:-false}"
telemetry = "${ENABLE_TELEMETRY:-true}"
beta_ui = "${BETA_FEATURES:-false}"

# Build and deployment info
[build]
version = "${BUILD_VERSION:-dev}"
commit = "${GIT_COMMIT:-unknown}"
timestamp = "${BUILD_TIMESTAMP:-unknown}"
environment = "${DEPLOY_ENV:-development}"
```

### YAML and JSON Substitution

Environment variable substitution works in all supported formats:

**YAML**:
```yaml
app:
  name: "${APP_NAME:-MyApp}"
  debug: "${DEBUG:-false}"

database:
  url: "${DATABASE_URL:-postgresql://localhost:5432/mydb}"

features:
  experimental: "${EXPERIMENTAL:-false}"
```

**JSON**:
```json
{
  "app": {
    "name": "${APP_NAME:-MyApp}",
    "debug": "${DEBUG:-false}"
  },
  "database": {
    "url": "${DATABASE_URL:-postgresql://localhost:5432/mydb}"
  },
  "features": {
    "experimental": "${EXPERIMENTAL:-false}"
  }
}
```

## Template Integration

Configuration values are automatically available in all Liquid templates through the `TemplateContext`.

### Basic Template Usage

Configuration values can be accessed directly in templates:

```liquid
# Project Configuration

**Application:** {{app.name}} v{{app.version}}
**Environment:** {{app.environment}}
**Debug Mode:** {% if app.debug %}enabled{% else %}disabled{% endif %}

## Database Settings

- **Host:** {{database.host}}:{{database.port}}
- **Database:** {{database.credentials.database}}
- **SSL:** {% if database.ssl_enabled %}✓ Enabled{% else %}✗ Disabled{% endif %}

## Feature Status

{% for feature in features -%}
{% if feature[1] %}✓ {{feature[0] | capitalize}} is enabled{% else %}✗ {{feature[0] | capitalize}} is disabled{% endif %}
{% endfor %}
```

### Advanced Template Patterns

```liquid
{% comment %}
  Generate environment-specific configuration
{% endcomment %}

# {{app.name | upcase}} Configuration

Environment: **{{app.environment | capitalize}}**

{% if app.environment == "development" -%}
## Development Settings
- Debug logging enabled
- Hot reload active
- Test database in use
{% elsif app.environment == "production" -%}
## Production Settings
- Optimized performance
- Error reporting enabled
- Production database active
{% endif %}

## Database Connection

```bash
# Connection string
{{database_url}}

# Individual components
HOST={{database.host}}
PORT={{database.port}}
DB={{database.credentials.database}}
USER={{database.credentials.username}}
```

## Features

{% assign enabled_features = features | where: "[1]", true -%}
{% assign disabled_features = features | where: "[1]", false -%}

### Enabled ({{enabled_features | size}})
{% for feature in enabled_features -%}
- ✓ {{feature[0] | capitalize | replace: "_", " "}}
{% endfor %}

{% if disabled_features.size > 0 -%}
### Disabled ({{disabled_features | size}})
{% for feature in disabled_features -%}
- ✗ {{feature[0] | capitalize | replace: "_", " "}}
{% endfor %}
{% endif %}

---
Generated at {{build.timestamp}} from commit {{build.commit}}
```

### Template Variables vs Configuration

You can combine configuration values with template-specific variables:

```liquid
{% comment %}
  Configuration provides: app.name, database.host, features
  Template variables provide: task, user, timestamp
{% endcomment %}

# Task Report for {{user}}

**Application:** {{app.name}}
**Task:** {{task}}
**Generated:** {{timestamp}}
**Database:** {{database.host}}

## Task Configuration

{% if features.advanced_reporting -%}
Advanced reporting features are available for this task.

### Available Features:
{% for feature in features -%}
{% if feature[1] -%}
- {{feature[0] | capitalize | replace: "_", " "}}
{% endif -%}
{% endfor %}
{% else -%}
Basic reporting mode active. Enable advanced_reporting in configuration for more features.
{% endif %}
```

## Model Configuration

SwissArmyHammer supports two AI execution models: **Claude Code** (default) and **LlamaAgent** with local Qwen models. Here are practical examples you can copy and use.

### Quick Start Examples

#### Default Claude Code Setup
Create `.swissarmyhammer/sah.toml`:
```toml
# Claude Code is the default - this is optional
[agent]
quiet = false

[agent.executor]
type = "claude-code"
```

#### Qwen Model Setup (Local AI)
Create `.swissarmyhammer/sah.toml`:
```toml
[agent]
quiet = false

[agent.executor]
type = "llama-agent"

[agent.executor.config.model.source.HuggingFace]
repo = "unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF"
filename = "Qwen3-Coder-30B-A3B-Instruct-UD-Q6_K_XL.gguf"

[agent.executor.config.mcp_server]
port = 0
timeout_seconds = 30
```

### Claude Code Configuration Options

**Basic Configuration:**
```toml
[agent.executor]
type = "claude-code"
```

**Custom Claude CLI Path:**
```toml
[agent.executor]
type = "claude-code"

[agent.executor.config]
claude_path = "/usr/local/bin/claude"
args = ["--timeout=60"]
```

**Environment Variables:**
```bash
export SAH_AGENT_EXECUTOR_TYPE="claude-code"
export SAH_AGENT_EXECUTOR_CONFIG_CLAUDE_PATH="/custom/path/to/claude"
```

### Qwen Model Configuration Options

#### Production Model (Qwen3-Coder-30B)
```toml
[agent.executor]
type = "llama-agent"

[agent.executor.config.model.source.HuggingFace]
repo = "unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF"
filename = "Qwen3-Coder-30B-A3B-Instruct-UD-Q6_K_XL.gguf"

[agent.executor.config.mcp_server]
port = 8080  # or 0 for random port
timeout_seconds = 60
```

#### Development/Testing Model (Phi-4-mini)
```toml
[agent.executor]
type = "llama-agent"

[agent.executor.config.model.source.HuggingFace]
repo = "unsloth/Phi-4-mini-instruct-GGUF"
filename = "Phi-4-mini-instruct-Q4_K_M.gguf"

[agent.executor.config.mcp_server]
port = 0
timeout_seconds = 10
```

#### Local Model File
```toml
[agent.executor]
type = "llama-agent"

[agent.executor.config.model.source.Local]
filename = "/path/to/your/model.gguf"

[agent.executor.config.mcp_server]
port = 0
timeout_seconds = 30
```

### Complete Configuration Examples

#### `.swissarmyhammer/sah.toml` - Claude Code
```toml
# Project configuration
project_name = "MyProject"

# Use Claude Code (default)
[agent]
quiet = false

[agent.executor]
type = "claude-code"

[agent.executor.config]
args = ["--timeout=120"]

# Other project settings
[app]
name = "MyProject"
version = "1.0.0"
```

#### `.swissarmyhammer/sah.toml` - Local Qwen Model
```toml
# Project configuration  
project_name = "MyProject"

# Use local Qwen model
[agent]
quiet = false

[agent.executor]
type = "llama-agent"

[agent.executor.config.model.source.HuggingFace]
repo = "unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF"
filename = "Qwen3-Coder-30B-A3B-Instruct-UD-Q6_K_XL.gguf"

[agent.executor.config.mcp_server]
port = 0
timeout_seconds = 30

# Other project settings
[app]
name = "MyProject" 
version = "1.0.0"
```

### Environment Variable Configuration

```bash
# Claude Code
export SAH_AGENT_EXECUTOR_TYPE="claude-code"
export SAH_AGENT_QUIET="false"

# Qwen Model
export SAH_AGENT_EXECUTOR_TYPE="llama-agent"
export SAH_AGENT_EXECUTOR_CONFIG_MODEL_SOURCE_HUGGINGFACE_REPO="unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF"
export SAH_AGENT_EXECUTOR_CONFIG_MODEL_SOURCE_HUGGINGFACE_FILENAME="Qwen3-Coder-30B-A3B-Instruct-UD-Q6_K_XL.gguf"
export SAH_AGENT_EXECUTOR_CONFIG_MCP_SERVER_PORT="0"
export SAH_AGENT_EXECUTOR_CONFIG_MCP_SERVER_TIMEOUT_SECONDS="30"
```

### Model Selection Guide

| Model | Size | RAM | Speed | Use Case |
|-------|------|-----|-------|-----------|
| **Claude Code** | N/A | Low | Fast | Production, cloud, general use |
| **Qwen3-Coder-30B** | ~18GB | ~20GB | Medium | Local development, privacy |  
| **Phi-4-mini** | ~1.5GB | ~2GB | Fast | Testing, limited resources |

### YAML Configuration Examples

#### Claude Code (YAML)
```yaml
agent:
  quiet: false
  executor:
    type: claude-code
    config:
      claude_path: "/usr/local/bin/claude"
      args: ["--timeout=60"]
```

#### Qwen Model (YAML)  
```yaml
agent:
  quiet: false
  executor:
    type: llama-agent
    config:
      model:
        source:
          HuggingFace:
            repo: "unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF"
            filename: "Qwen3-Coder-30B-A3B-Instruct-UD-Q6_K_XL.gguf"
      mcp_server:
        port: 0
        timeout_seconds: 30
```

### Testing Your Configuration

```bash
# Test configuration loading
sah doctor

# Check which model is active
sah config get agent.executor.type

# Test agent execution (if configured)
echo "Test prompt" | sah workflow run test-agent
```

## Advanced Usage

### Dynamic Configuration Loading

The configuration system loads fresh values on each access, allowing for runtime updates:

```rust
use swissarmyhammer_config::TemplateContext;

// Load configuration (no caching)
let context = TemplateContext::load()?;

// Access nested values
if let Some(debug) = context.get("app.debug") {
    println!("Debug mode: {}", debug);
}

// Check for environment-specific settings
if let Some(env) = context.get("app.environment") {
    match env.as_str().unwrap_or("development") {
        "production" => /* production settings */,
        "development" => /* development settings */,
        _ => /* default settings */,
    }
}
```

### Configuration with Template Variables

Combine configuration with runtime template variables:

```rust
use swissarmyhammer_config::TemplateContext;
use std::collections::HashMap;
use serde_json::json;

// Create template variables
let mut template_vars = HashMap::new();
template_vars.insert("task".to_string(), json!("deploy"));
template_vars.insert("user".to_string(), json!("admin"));
template_vars.insert("timestamp".to_string(), json!("2024-01-15T10:30:00Z"));

// Load configuration with template variables (template vars override config)
let context = TemplateContext::with_template_vars(template_vars)?;

// Template variables have highest precedence
assert_eq!(context.get("task"), Some(&json!("deploy")));
// Configuration values are still available
assert_eq!(context.get("app.name"), Some(&json!("MyProject")));
```

### CLI Integration

Configuration is automatically available in CLI contexts:

```rust
use swissarmyhammer_config::load_configuration_for_cli;

// Load configuration for CLI usage (bypasses security restrictions)
let context = load_configuration_for_cli()?;

// Use in CLI commands
println!("Project: {}", context.get("app.name").unwrap_or(&json!("Unknown")));
```



## Troubleshooting

### Common Issues

#### Configuration Not Loading

**Problem**: Configuration values not appearing in templates.

**Solutions**:
1. Check file discovery paths:
   ```bash
   # Verify files exist in correct locations
   ls -la ~/.swissarmyhammer/
   ls -la ./.swissarmyhammer/
   ```

2. Verify file format syntax:
   ```bash
   # Test TOML syntax
   toml-lint ~/.swissarmyhammer/sah.toml

   # Test YAML syntax
   yamllint ~/.swissarmyhammer/sah.yaml

   # Test JSON syntax
   jq . ~/.swissarmyhammer/sah.json
   ```

3. Check configuration loading:
   ```bash
   sah doctor  # Should show loaded configuration
   ```

#### Environment Variables Not Working

**Problem**: Environment variables not overriding configuration files.

**Solutions**:
1. Check variable naming:
   ```bash
   # Correct naming
   export SAH_APP_NAME="MyApp"           # → app.name
   export SAH_DATABASE_HOST="localhost"  # → database.host

   # Incorrect naming (won't work)
   export APP_NAME="MyApp"               # Missing SAH_ prefix
   export SAH_APP-NAME="MyApp"           # Hyphens not supported
   ```

2. Verify environment variable visibility:
   ```bash
   env | grep SAH_
   env | grep SWISSARMYHAMMER_
   ```

3. Test type conversion:
   ```bash
   # Boolean values
   export SAH_DEBUG="true"    # Correct
   export SAH_DEBUG="True"    # Also works
   export SAH_DEBUG="yes"     # Won't convert to boolean

   # Numeric values
   export SAH_PORT="5432"     # Correct
   export SAH_PORT="5432.0"   # Also works
   export SAH_PORT="5432px"   # Won't convert to number
   ```

#### Environment Variable Substitution Failures

**Problem**: `${VAR}` placeholders not being replaced in configuration files.

**Solutions**:
1. Check environment variable existence:
   ```bash
   echo $DATABASE_URL  # Should show value
   ```

2. Verify substitution syntax:
   ```toml
   # Correct syntax
   url = "${DATABASE_URL}"
   url = "${DATABASE_URL:-default_value}"

   # Incorrect syntax
   url = "$DATABASE_URL"        # Missing braces
   url = "${DATABASE_URL-}"     # Wrong default syntax
   ```

3. Check nested substitution:
   ```toml
   # Works
   url = "${DB_URL:-postgresql://${DB_USER}:${DB_PASS}@localhost:5432/mydb}"

   # Complex nesting may need simplification
   url = "${COMPLEX_URL:-${NESTED_${DYNAMIC}_VAR}}"  # May not work
   ```

#### Precedence Issues

**Problem**: Configuration values not overriding as expected.

**Solutions**:
1. Understand precedence order (lowest to highest):
   - Default values
   - Global config (`~/.swissarmyhammer/`)
   - Project config (`./.swissarmyhammer/`)
   - Environment variables
   - CLI arguments

2. Check all sources:
   ```bash
   # Check global config
   cat ~/.swissarmyhammer/sah.toml

   # Check project config
   cat ./.swissarmyhammer/sah.toml

   # Check environment variables
   env | grep SAH_
   ```

3. Use configuration diagnosis:
   ```bash
   sah doctor  # Shows final merged configuration
   ```

### Debugging Tips

1. **Use `sah doctor`**: Shows loaded configuration and sources
2. **Check file permissions**: Ensure configuration files are readable
3. **Validate syntax**: Use format-specific linters
4. **Test incrementally**: Start with simple configuration and add complexity
5. **Check logs**: Enable debug logging to see configuration loading process

### Getting Help

If you encounter issues not covered here:

1. Check the [GitHub Issues](https://github.com/swissarmyhammer/swissarmyhammer/issues)
2. Review the [API documentation](https://docs.rs/swissarmyhammer-config)
3. Join our community discussions
4. Create a minimal reproduction case when reporting bugs

---

This configuration system provides powerful, flexible configuration management while maintaining simplicity for basic use cases. The combination of multiple formats, environment integration, and template availability makes it suitable for everything from simple personal projects to complex enterprise deployments.
