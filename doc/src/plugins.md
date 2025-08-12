# Plugin Development

SwissArmyHammer features a flexible plugin architecture that allows you to extend functionality through custom filters, processors, and integrations. The plugin system enables seamless integration of external tools and custom processing logic.

## Overview

The plugin system provides:
- **Custom Filters**: Transform and process prompt content
- **Processing Plugins**: Add new data processing capabilities  
- **Template Extensions**: Extend the Liquid template engine
- **Integration Plugins**: Connect with external tools and services
- **Workflow Actions**: Create custom workflow step implementations

## Plugin Architecture

### Core Components

**Plugin Interface**: All plugins implement a common interface
```rust
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn process(&self, input: &str, context: &PluginContext) -> PluginResult<String>;
}
```

**Plugin Registry**: Central management of available plugins
```rust  
pub struct PluginRegistry {
    plugins: HashMap<String, Box<dyn Plugin>>,
}
```

**Plugin Context**: Provides contextual information to plugins
```rust
pub struct PluginContext {
    pub file_path: Option<String>,
    pub language: Option<String>,
    pub metadata: HashMap<String, String>,
    pub environment: HashMap<String, String>,
}
```

### Plugin Types

**Filter Plugins**: Transform text content
- Input processing and validation
- Output formatting and styling
- Content transformation and encoding

**Data Plugins**: Process structured data
- File format conversions
- Data extraction and parsing
- External API integrations

**Workflow Plugins**: Custom workflow actions
- External tool execution
- Conditional logic and branching
- State management and persistence

## Built-in Plugins

### Text Processing Filters

**Trim Filter**: Remove whitespace
```rust
use swissarmyhammer::prompt_filter::PromptFilter;

let filter = PromptFilter::Trim;
let result = filter.apply("  hello world  ")?;
// Result: "hello world"
```

**Case Transformation**:
```rust
let uppercase = PromptFilter::Uppercase;
let lowercase = PromptFilter::Lowercase;
let titlecase = PromptFilter::TitleCase;

let result = uppercase.apply("hello world")?;
// Result: "HELLO WORLD"
```

**String Manipulation**:
```rust
let replace = PromptFilter::Replace {
    pattern: "old".to_string(),
    replacement: "new".to_string(),
};

let result = replace.apply("old text with old words")?;
// Result: "new text with new words"
```

### Code Processing Filters

**Syntax Highlighting**:
```rust
let highlight = PromptFilter::CodeHighlight {
    language: "rust".to_string(),
};

let code = r#"
fn main() {
    println!("Hello, world!");
}
"#;

let result = highlight.apply(code)?;
// Result: HTML with syntax highlighting
```

**Code Formatting**:
```rust  
let format = PromptFilter::CodeFormat {
    language: "rust".to_string(),
    style: FormatStyle::Standard,
};

let result = format.apply(unformatted_code)?;
```

### File System Filters

**File Reading**:
```rust
let read_file = PromptFilter::FileRead {
    path: "./src/main.rs".to_string(),
};

let content = read_file.apply("")?;
// Result: File contents
```

**Directory Listing**:
```rust
let list_files = PromptFilter::ListFiles {
    path: "./src".to_string(),
    pattern: Some("*.rs".to_string()),
};

let files = list_files.apply("")?;
// Result: Newline-separated file paths
```

### External Tool Integration

**Shell Command Execution**:
```rust
let shell = PromptFilter::Shell {
    command: "git log --oneline -5".to_string(),
    timeout: Some(10),
};

let output = shell.apply("")?;
// Result: Command output
```

**HTTP Requests**:
```rust
let http = PromptFilter::HttpGet {
    url: "https://api.example.com/data".to_string(),
    headers: HashMap::new(),
    timeout: Some(30),
};

let response = http.apply("")?;
// Result: HTTP response body
```

## Creating Custom Plugins

### Basic Plugin Implementation

```rust
use swissarmyhammer::plugins::{Plugin, PluginContext, PluginResult};

#[derive(Debug)]
pub struct ReverseStringPlugin;

impl Plugin for ReverseStringPlugin {
    fn name(&self) -> &str {
        "reverse"
    }
    
    fn description(&self) -> &str {
        "Reverses the input string"
    }
    
    fn process(&self, input: &str, _context: &PluginContext) -> PluginResult<String> {
        Ok(input.chars().rev().collect())
    }
}

// Usage in templates:
// {{ content | reverse }}
```

### Advanced Plugin with Context

```rust
use swissarmyhammer::plugins::*;
use std::fs;

#[derive(Debug)]
pub struct ProjectInfoPlugin;

impl Plugin for ProjectInfoPlugin {
    fn name(&self) -> &str {
        "project_info"
    }
    
    fn description(&self) -> &str {
        "Extracts project information from context"
    }
    
    fn process(&self, _input: &str, context: &PluginContext) -> PluginResult<String> {
        let mut info = Vec::new();
        
        // Get language from context
        if let Some(lang) = &context.language {
            info.push(format!("Language: {}", lang));
        }
        
        // Get file path info
        if let Some(path) = &context.file_path {
            if let Some(name) = std::path::Path::new(path).file_name() {
                info.push(format!("File: {}", name.to_string_lossy()));
            }
        }
        
        // Check for project files
        if std::path::Path::new("Cargo.toml").exists() {
            info.push("Project: Rust".to_string());
        } else if std::path::Path::new("package.json").exists() {
            info.push("Project: Node.js".to_string());
        }
        
        Ok(info.join("\n"))
    }
}
```

### Error Handling in Plugins

```rust
use swissarmyhammer::plugins::*;

#[derive(Debug)]
pub struct ValidatingPlugin;

impl Plugin for ValidatingPlugin {
    fn name(&self) -> &str {
        "validate_json"
    }
    
    fn description(&self) -> &str {
        "Validates and formats JSON content"
    }
    
    fn process(&self, input: &str, _context: &PluginContext) -> PluginResult<String> {
        match serde_json::from_str::<serde_json::Value>(input) {
            Ok(value) => {
                // Format with indentation
                match serde_json::to_string_pretty(&value) {
                    Ok(formatted) => Ok(formatted),
                    Err(e) => Err(PluginError::ProcessingError {
                        message: format!("Failed to format JSON: {}", e),
                        source: Some(Box::new(e)),
                    }),
                }
            }
            Err(e) => Err(PluginError::ValidationError {
                message: format!("Invalid JSON: {}", e),
                input_excerpt: input.chars().take(100).collect(),
            }),
        }
    }
}
```

## Plugin Registration and Usage

### Registering Plugins

```rust
use swissarmyhammer::plugins::PluginRegistry;

// Create registry with built-in plugins
let mut registry = PluginRegistry::with_builtin_plugins();

// Register custom plugins
registry.register(Box::new(ReverseStringPlugin))?;
registry.register(Box::new(ProjectInfoPlugin))?;
registry.register(Box::new(ValidatingPlugin))?;

// Use in prompt library
let library = PromptLibrary::new()
    .with_plugin_registry(registry);
```

### Using Plugins in Templates

```liquid
<!-- Basic usage -->
{{ content | reverse }}

<!-- Chaining filters -->
{{ code | trim | code_highlight: "rust" | reverse }}

<!-- With parameters -->
{{ json_data | validate_json }}

<!-- Conditional usage -->
{% if language == "rust" %}
{{ code | code_format: "rust" }}
{% else %}
{{ code | trim }}
{% endif %}

<!-- Complex processing -->
{{ file_path | file_read | code_highlight: language | trim }}
```

### Dynamic Plugin Loading

```rust
use swissarmyhammer::plugins::{PluginLoader, PluginConfig};

// Load plugins from directory
let loader = PluginLoader::new();
let plugins = loader.load_from_directory("./plugins")?;

// Load with configuration
let config = PluginConfig {
    allow_unsafe: false,
    timeout: Some(30),
    memory_limit: Some(100 * 1024 * 1024), // 100MB
    ..Default::default()
};

let plugins = loader.load_with_config("./plugins", config)?;

// Register loaded plugins
for plugin in plugins {
    registry.register(plugin)?;
}
```

## Advanced Plugin Patterns

### Stateful Plugins

```rust
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

#[derive(Debug)]
pub struct CachingPlugin {
    cache: Arc<Mutex<HashMap<String, String>>>,
}

impl CachingPlugin {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Plugin for CachingPlugin {
    fn name(&self) -> &str {
        "cache"
    }
    
    fn description(&self) -> &str {
        "Caches expensive computations"
    }
    
    fn process(&self, input: &str, context: &PluginContext) -> PluginResult<String> {
        let cache_key = format!("{}:{}", input, context.file_path.as_deref().unwrap_or(""));
        
        // Check cache first
        {
            let cache = self.cache.lock().unwrap();
            if let Some(cached) = cache.get(&cache_key) {
                return Ok(cached.clone());
            }
        }
        
        // Expensive computation
        let result = expensive_computation(input)?;
        
        // Store in cache
        {
            let mut cache = self.cache.lock().unwrap();
            cache.insert(cache_key, result.clone());
        }
        
        Ok(result)
    }
}
```

### Async Plugin Processing

```rust
use tokio::runtime::Runtime;

#[derive(Debug)]
pub struct AsyncPlugin {
    runtime: Runtime,
}

impl AsyncPlugin {
    pub fn new() -> PluginResult<Self> {
        let runtime = Runtime::new()
            .map_err(|e| PluginError::InitializationError {
                message: format!("Failed to create async runtime: {}", e),
                source: Some(Box::new(e)),
            })?;
        
        Ok(Self { runtime })
    }
    
    async fn async_process(&self, input: &str) -> Result<String, Box<dyn std::error::Error>> {
        // Async operations like HTTP requests, database queries, etc.
        let client = reqwest::Client::new();
        let response = client
            .post("https://api.example.com/process")
            .body(input.to_string())
            .send()
            .await?
            .text()
            .await?;
        
        Ok(response)
    }
}

impl Plugin for AsyncPlugin {
    fn name(&self) -> &str {
        "async_processor"
    }
    
    fn description(&self) -> &str {
        "Processes input asynchronously"
    }
    
    fn process(&self, input: &str, _context: &PluginContext) -> PluginResult<String> {
        self.runtime.block_on(self.async_process(input))
            .map_err(|e| PluginError::ProcessingError {
                message: format!("Async processing failed: {}", e),
                source: Some(e),
            })
    }
}
```

### Configuration-Based Plugins

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct DatabaseConfig {
    pub connection_string: String,
    pub timeout: u64,
    pub pool_size: u32,
}

#[derive(Debug)]
pub struct DatabasePlugin {
    config: DatabaseConfig,
    // connection pool, etc.
}

impl DatabasePlugin {
    pub fn new(config: DatabaseConfig) -> PluginResult<Self> {
        // Initialize database connection
        Ok(Self { config })
    }
    
    pub fn from_config_file<P: AsRef<std::path::Path>>(path: P) -> PluginResult<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| PluginError::ConfigError {
                message: format!("Failed to read config file: {}", e),
                source: Some(Box::new(e)),
            })?;
        
        let config: DatabaseConfig = toml::from_str(&content)
            .map_err(|e| PluginError::ConfigError {
                message: format!("Failed to parse config: {}", e),
                source: Some(Box::new(e)),
            })?;
        
        Self::new(config)
    }
}

impl Plugin for DatabasePlugin {
    fn name(&self) -> &str {
        "database_query"
    }
    
    fn description(&self) -> &str {
        "Executes database queries"
    }
    
    fn process(&self, input: &str, _context: &PluginContext) -> PluginResult<String> {
        // Execute SQL query and return results
        // Implementation depends on database driver
        todo!("Implement database query execution")
    }
}
```

## Testing Plugins

### Unit Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer::plugins::PluginContext;
    
    #[test]
    fn test_reverse_plugin() {
        let plugin = ReverseStringPlugin;
        let context = PluginContext::default();
        
        let result = plugin.process("hello", &context).unwrap();
        assert_eq!(result, "olleh");
    }
    
    #[test]
    fn test_plugin_with_context() {
        let plugin = ProjectInfoPlugin;
        let context = PluginContext {
            language: Some("rust".to_string()),
            file_path: Some("src/main.rs".to_string()),
            ..Default::default()
        };
        
        let result = plugin.process("", &context).unwrap();
        assert!(result.contains("Language: rust"));
        assert!(result.contains("File: main.rs"));
    }
    
    #[test]
    fn test_error_handling() {
        let plugin = ValidatingPlugin;
        let context = PluginContext::default();
        
        // Valid JSON
        let valid_json = r#"{"name": "test"}"#;
        let result = plugin.process(valid_json, &context);
        assert!(result.is_ok());
        
        // Invalid JSON
        let invalid_json = r#"{"name": "test""#;
        let result = plugin.process(invalid_json, &context);
        assert!(result.is_err());
    }
}
```

### Integration Testing

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use swissarmyhammer::prelude::*;
    
    #[test]
    fn test_plugin_in_template() {
        let mut registry = PluginRegistry::new();
        registry.register(Box::new(ReverseStringPlugin)).unwrap();
        
        let library = PromptLibrary::new()
            .with_plugin_registry(registry);
        
        let template = "{{ content | reverse }}";
        let context = HashMap::from([
            ("content".to_string(), "hello world".to_string()),
        ]);
        
        let result = library.render_template(template, &context).unwrap();
        assert_eq!(result, "dlrow olleh");
    }
}
```

## Best Practices

### Plugin Development

**Error Handling**:
- Use descriptive error messages
- Provide context about what went wrong
- Include suggestions for fixing issues
- Handle edge cases gracefully

**Performance**:
- Cache expensive computations
- Use appropriate data structures
- Implement timeout mechanisms
- Monitor memory usage

**Security**:
- Validate all inputs
- Sanitize file paths
- Limit resource usage
- Avoid executing arbitrary code

### Plugin Distribution

**Documentation**:
- Provide clear usage examples
- Document configuration options
- Include troubleshooting guides
- Maintain API compatibility

**Packaging**:
```toml
# Cargo.toml for plugin crate
[package]
name = "sah-plugin-example"
version = "0.1.0"

[dependencies]
swissarmyhammer = "0.1"
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1.0", optional = true }

[features]
default = []
async = ["tokio"]
```

**Plugin Manifest**:
```toml
# plugin.toml
[plugin]
name = "example-plugin"
version = "0.1.0"
description = "Example plugin for SwissArmyHammer"
author = "Your Name <email@example.com>"

[plugin.capabilities]
filters = ["reverse", "project_info"]
processors = ["validate_json"]

[plugin.requirements]
min_sah_version = "0.1.0"
features = ["async"]
```

## Plugin Ecosystem

### Community Plugins

Popular community-developed plugins:

**Development Tools**:
- Code formatters and linters
- Git integration plugins
- CI/CD workflow helpers
- Documentation generators

**External Integrations**:
- API clients for popular services
- Database connectors
- Cloud platform integrations
- Monitoring and logging tools

**Content Processing**:
- Markdown processors
- Image manipulation tools
- Data format converters
- Template engines

### Plugin Registry

```bash
# Install plugins from registry
sah plugin install reverse-string
sah plugin install database-query

# List installed plugins
sah plugin list

# Update plugins
sah plugin update

# Remove plugins
sah plugin remove reverse-string
```

## Troubleshooting

### Common Issues

**Plugin Not Found**:
- Verify plugin is registered in registry
- Check plugin name spelling
- Ensure plugin is loaded before use

**Processing Errors**:
- Check plugin logs for error details
- Validate input data format
- Verify plugin configuration
- Test plugin in isolation

**Performance Issues**:
- Profile plugin execution time
- Check for memory leaks
- Optimize expensive operations
- Implement caching where appropriate

### Debug Mode

```rust
use swissarmyhammer::plugins::{PluginRegistry, DebugConfig};

let debug_config = DebugConfig {
    enable_logging: true,
    log_level: LogLevel::Debug,
    trace_execution: true,
    dump_context: true,
};

let registry = PluginRegistry::with_debug(debug_config);
```

The plugin system enables unlimited extensibility of SwissArmyHammer, allowing you to integrate with any tool, service, or processing pipeline while maintaining type safety and performance.