use std::env;
use tracing::{debug, info, warn};

/// Re-export for convenience
pub use crate::cli_builder::DynamicCommandInfo;

/// CLI debugger for troubleshooting dynamic CLI issues
pub struct CliDebugger {
    enabled: bool,
    verbose: bool,
}

impl CliDebugger {
    /// Create new CLI debugger
    pub fn new() -> Self {
        let enabled = env::var("SAH_CLI_DEBUG").is_ok() || cfg!(debug_assertions);
        let verbose = env::var("SAH_CLI_DEBUG_VERBOSE").is_ok();
        
        Self { enabled, verbose }
    }

    /// Log command parsing process
    pub fn log_command_parsing(&self, args: &[String]) {
        if self.enabled {
            info!("🔍 Parsing CLI arguments: {:?}", args);
            if self.verbose {
                debug!("Arguments count: {}", args.len());
                for (i, arg) in args.iter().enumerate() {
                    debug!("  arg[{}]: '{}'", i, arg);
                }
            }
        }
    }

    /// Log dynamic command detection
    pub fn log_dynamic_command_detection(&self, command_info: &DynamicCommandInfo) {
        if self.enabled {
            info!("✅ Detected dynamic command: {:?}", command_info);
            if self.verbose {
                debug!("Category: {:?}", command_info.category);
                debug!("Tool name: {}", command_info.tool_name);
                debug!("MCP tool name: {}", command_info.mcp_tool_name);
            }
        }
    }

    /// Log schema conversion process
    pub fn log_schema_conversion(&self, tool_name: &str, schema: &serde_json::Value) {
        if self.enabled {
            info!("🔧 Converting schema for tool: {}", tool_name);
            if self.verbose {
                debug!("Schema for {}: {}", tool_name, 
                    serde_json::to_string_pretty(schema).unwrap_or_else(|_| "Invalid JSON".to_string()));
            }
        }
    }

    /// Log MCP tool execution
    pub fn log_mcp_tool_execution(&self, tool_name: &str, arguments: &serde_json::Map<String, serde_json::Value>) {
        if self.enabled {
            info!("🚀 Executing MCP tool: {}", tool_name);
            if self.verbose && !arguments.is_empty() {
                debug!("Arguments for {}: {}", tool_name,
                    serde_json::to_string_pretty(arguments).unwrap_or_else(|_| "Invalid JSON".to_string()));
            }
        }
    }

    /// Log CLI builder creation
    pub fn log_cli_builder_creation(&self, tool_count: usize, categories: &[String]) {
        if self.enabled {
            info!("🏗️  Building dynamic CLI with {} tools in {} categories", tool_count, categories.len());
            if self.verbose {
                debug!("Categories: {:?}", categories);
            }
        }
    }

    /// Log CLI caching events
    pub fn log_cli_cache_event(&self, event: CacheEvent) {
        if self.enabled {
            match event {
                CacheEvent::Hit => info!("⚡ CLI cache hit - using cached CLI structure"),
                CacheEvent::Miss => info!("💾 CLI cache miss - building new CLI structure"),
                CacheEvent::Build { duration_ms } => {
                    info!("🔨 CLI build completed in {}ms", duration_ms);
                },
                CacheEvent::Error(err) => warn!("❌ CLI cache error: {}", err),
            }
        }
    }

    /// Log MCP infrastructure initialization
    pub fn log_mcp_infrastructure_init(&self, event: McpInitEvent) {
        if self.enabled {
            match event {
                McpInitEvent::Starting => info!("🔧 Initializing MCP infrastructure"),
                McpInitEvent::Success { duration_ms, tool_count } => {
                    info!("✅ MCP infrastructure ready in {}ms with {} tools", duration_ms, tool_count);
                },
                McpInitEvent::Warning(msg) => warn!("⚠️  MCP infrastructure warning: {}", msg),
                McpInitEvent::Error(err) => warn!("❌ MCP infrastructure error: {}", err),
                McpInitEvent::Timeout { timeout_secs } => {
                    warn!("⏰ MCP infrastructure timed out after {}s", timeout_secs);
                },
            }
        }
    }

    /// Log CLI execution path decisions
    pub fn log_execution_path(&self, path: ExecutionPath) {
        if self.enabled {
            match path {
                ExecutionPath::StaticCommand { command } => {
                    info!("🔀 Using static command path: {}", command);
                },
                ExecutionPath::DynamicCommand { tool_name, category } => {
                    info!("🔀 Using dynamic command path: {} ({})", tool_name, 
                          category.unwrap_or("no category".to_string()));
                },
                ExecutionPath::Fallback { reason } => {
                    info!("🔀 Falling back to static CLI: {}", reason);
                },
                ExecutionPath::FastPath { command_type } => {
                    info!("⚡ Using fast path for: {}", command_type);
                },
            }
        }
    }

    /// Log help text generation
    pub fn log_help_generation(&self, context: &str, generated: bool) {
        if self.enabled {
            if generated {
                info!("📝 Generated help text for: {}", context);
            } else {
                debug!("📝 Using static help text for: {}", context);
            }
        }
    }

    /// Log validation events
    pub fn log_validation(&self, validation: ValidationEvent) {
        if self.enabled {
            match validation {
                ValidationEvent::SchemaValid { tool_name } => {
                    debug!("✅ Schema validation passed for: {}", tool_name);
                },
                ValidationEvent::SchemaInvalid { tool_name, error } => {
                    warn!("❌ Schema validation failed for {}: {}", tool_name, error);
                },
                ValidationEvent::ArgumentValid { arg_name } => {
                    debug!("✅ Argument validation passed for: {}", arg_name);
                },
                ValidationEvent::ArgumentInvalid { arg_name, error } => {
                    warn!("❌ Argument validation failed for {}: {}", arg_name, error);
                },
            }
        }
    }

    /// Check if debugging is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Check if verbose debugging is enabled
    pub fn is_verbose(&self) -> bool {
        self.verbose
    }
}

impl Default for CliDebugger {
    fn default() -> Self {
        Self::new()
    }
}

/// Cache-related events
#[derive(Debug, Clone)]
pub enum CacheEvent {
    Hit,
    Miss,
    Build { duration_ms: u64 },
    Error(String),
}

/// MCP infrastructure initialization events
#[derive(Debug, Clone)]
pub enum McpInitEvent {
    Starting,
    Success { duration_ms: u64, tool_count: usize },
    Warning(String),
    Error(String),
    Timeout { timeout_secs: u64 },
}

/// CLI execution path decisions
#[derive(Debug, Clone)]
pub enum ExecutionPath {
    StaticCommand { command: String },
    DynamicCommand { tool_name: String, category: Option<String> },
    Fallback { reason: String },
    FastPath { command_type: String },
}

/// Validation events
#[derive(Debug, Clone)]
pub enum ValidationEvent {
    SchemaValid { tool_name: String },
    SchemaInvalid { tool_name: String, error: String },
    ArgumentValid { arg_name: String },
    ArgumentInvalid { arg_name: String, error: String },
}

/// Global debugger instance
pub static DEBUGGER: std::sync::LazyLock<CliDebugger> = std::sync::LazyLock::new(CliDebugger::new);

/// Convenience function to get the global debugger
pub fn debugger() -> &'static CliDebugger {
    &DEBUGGER
}

/// Macro for conditional debug logging
#[macro_export]
macro_rules! cli_debug {
    ($($arg:tt)*) => {
        if $crate::debug::debugger().is_enabled() {
            tracing::debug!($($arg)*);
        }
    };
}

/// Macro for conditional info logging
#[macro_export]
macro_rules! cli_info {
    ($($arg:tt)*) => {
        if $crate::debug::debugger().is_enabled() {
            tracing::info!($($arg)*);
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_debugger_creation() {
        let debugger = CliDebugger::new();
        
        // Should be enabled in debug builds or with env var
        let expected_enabled = cfg!(debug_assertions) || env::var("SAH_CLI_DEBUG").is_ok();
        assert_eq!(debugger.is_enabled(), expected_enabled);
        
        // Verbose should depend on specific env var
        let expected_verbose = env::var("SAH_CLI_DEBUG_VERBOSE").is_ok();
        assert_eq!(debugger.is_verbose(), expected_verbose);
    }

    #[test]
    fn test_cache_events() {
        let debugger = CliDebugger::new();
        
        // These should not panic
        debugger.log_cli_cache_event(CacheEvent::Hit);
        debugger.log_cli_cache_event(CacheEvent::Miss);
        debugger.log_cli_cache_event(CacheEvent::Build { duration_ms: 100 });
        debugger.log_cli_cache_event(CacheEvent::Error("test error".to_string()));
    }

    #[test]
    fn test_mcp_init_events() {
        let debugger = CliDebugger::new();
        
        // These should not panic
        debugger.log_mcp_infrastructure_init(McpInitEvent::Starting);
        debugger.log_mcp_infrastructure_init(McpInitEvent::Success { duration_ms: 500, tool_count: 10 });
        debugger.log_mcp_infrastructure_init(McpInitEvent::Warning("test warning".to_string()));
        debugger.log_mcp_infrastructure_init(McpInitEvent::Error("test error".to_string()));
        debugger.log_mcp_infrastructure_init(McpInitEvent::Timeout { timeout_secs: 30 });
    }

    #[test]
    fn test_execution_path_events() {
        let debugger = CliDebugger::new();
        
        // These should not panic
        debugger.log_execution_path(ExecutionPath::StaticCommand { 
            command: "serve".to_string() 
        });
        debugger.log_execution_path(ExecutionPath::DynamicCommand { 
            tool_name: "issue_create".to_string(),
            category: Some("issue".to_string())
        });
        debugger.log_execution_path(ExecutionPath::Fallback { 
            reason: "MCP timeout".to_string() 
        });
        debugger.log_execution_path(ExecutionPath::FastPath { 
            command_type: "help".to_string() 
        });
    }

    #[test]
    fn test_validation_events() {
        let debugger = CliDebugger::new();
        
        // These should not panic
        debugger.log_validation(ValidationEvent::SchemaValid { 
            tool_name: "test_tool".to_string() 
        });
        debugger.log_validation(ValidationEvent::SchemaInvalid { 
            tool_name: "test_tool".to_string(),
            error: "invalid format".to_string()
        });
        debugger.log_validation(ValidationEvent::ArgumentValid { 
            arg_name: "test_arg".to_string() 
        });
        debugger.log_validation(ValidationEvent::ArgumentInvalid { 
            arg_name: "test_arg".to_string(),
            error: "invalid value".to_string()
        });
    }

    #[test]
    fn test_command_parsing_log() {
        let debugger = CliDebugger::new();
        let args = vec!["sah".to_string(), "issue".to_string(), "create".to_string()];
        
        // Should not panic
        debugger.log_command_parsing(&args);
    }

    #[test]
    fn test_schema_conversion_log() {
        let debugger = CliDebugger::new();
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            }
        });
        
        // Should not panic
        debugger.log_schema_conversion("test_tool", &schema);
    }

    #[test]
    fn test_mcp_tool_execution_log() {
        let debugger = CliDebugger::new();
        let mut arguments = serde_json::Map::new();
        arguments.insert("name".to_string(), serde_json::Value::String("test".to_string()));
        
        // Should not panic
        debugger.log_mcp_tool_execution("test_tool", &arguments);
    }

    #[test]
    fn test_cli_builder_creation_log() {
        let debugger = CliDebugger::new();
        let categories = vec!["issue".to_string(), "memo".to_string()];
        
        // Should not panic
        debugger.log_cli_builder_creation(10, &categories);
    }

    #[test]
    fn test_help_generation_log() {
        let debugger = CliDebugger::new();
        
        // Should not panic
        debugger.log_help_generation("test_command", true);
        debugger.log_help_generation("static_command", false);
    }

    #[test]
    fn test_global_debugger() {
        let debugger1 = debugger();
        let debugger2 = debugger();
        
        // Should return the same instance
        assert!(std::ptr::eq(debugger1, debugger2));
    }
}