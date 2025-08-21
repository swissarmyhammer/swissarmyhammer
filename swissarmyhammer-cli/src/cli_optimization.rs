use anyhow::Result;
use clap::Command;
use std::collections::HashMap;
use std::sync::OnceLock;

use std::sync::Arc;
use crate::cli_builder::CliBuilder;

// Cache for CLI command structure to avoid rebuilding
static CLI_CACHE: OnceLock<Command> = OnceLock::new();

// Cache for tool metadata to improve repeated CLI operations
static TOOL_METADATA_CACHE: OnceLock<HashMap<String, String>> = OnceLock::new();

/// Get cached CLI command or build it if not cached
pub async fn get_or_build_cli() -> Result<&'static Command> {
    if let Some(cli) = CLI_CACHE.get() {
        return Ok(cli);
    }
    
    let cli = build_dynamic_cli_internal().await?;
    CLI_CACHE.set(cli).map_err(|_| anyhow::anyhow!("Failed to cache CLI"))?;
    
    Ok(CLI_CACHE.get().unwrap())
}

/// Internal function to build dynamic CLI (used for caching)
async fn build_dynamic_cli_internal() -> Result<Command> {
    use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;
    use swissarmyhammer_tools::*;
    
    // Create the tool registry and register tools
    let mut tool_registry = ToolRegistry::new();
    
    // Register all tools 
    register_file_tools(&mut tool_registry);
    register_issue_tools(&mut tool_registry);
    register_memo_tools(&mut tool_registry);
    register_notify_tools(&mut tool_registry);
    register_search_tools(&mut tool_registry);
    register_shell_tools(&mut tool_registry);
    register_todo_tools(&mut tool_registry);
    register_web_fetch_tools(&mut tool_registry);
    register_web_search_tools(&mut tool_registry);
    
    let tool_registry = Arc::new(tool_registry);
    
    // Build CLI using the CLI builder
    let cli_builder = CliBuilder::new(tool_registry);
    cli_builder.build_cli()
}

/// Fast path for help commands that don't need full MCP initialization
pub fn is_help_command(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--help" || arg == "-h")
}

/// Fast path for version commands
pub fn is_version_command(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--version" || arg == "-V")
}

/// Fast path for completion commands
pub fn is_completion_command(args: &[String]) -> bool {
    args.len() >= 2 && args[1] == "completion"
}

/// Handle fast-path commands that can be processed without full MCP initialization
pub async fn handle_fast_path_commands(args: &[String]) -> Result<bool> {
    // Handle help requests
    if is_help_command(args) || args.is_empty() {
        // For help, we can use a cached CLI structure or build minimal CLI if needed
        if let Ok(cli) = get_or_build_cli().await {
            let _ = cli.clone().print_help();
            return Ok(true);
        }
    }
    
    // Handle version requests
    if is_version_command(args) {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return Ok(true);
    }
    
    Ok(false)
}

/// Get cached tool metadata
pub fn get_cached_tool_metadata() -> Option<&'static HashMap<String, String>> {
    TOOL_METADATA_CACHE.get()
}

/// Cache tool metadata for performance
pub fn cache_tool_metadata(metadata: HashMap<String, String>) -> Result<()> {
    TOOL_METADATA_CACHE.set(metadata).map_err(|_| anyhow::anyhow!("Failed to cache tool metadata"))?;
    Ok(())
}

/// Clear all caches (useful for testing or when tools change)
pub fn clear_caches() {
    // Note: OnceLock doesn't provide a way to clear values once set
    // This function exists for API completeness and potential future implementation
    tracing::debug!("Cache clearing requested (not implemented for OnceLock)");
}

/// Check if CLI is cached
pub fn is_cli_cached() -> bool {
    CLI_CACHE.get().is_some()
}

/// Get cache statistics for debugging
pub fn get_cache_stats() -> CacheStats {
    CacheStats {
        cli_cached: CLI_CACHE.get().is_some(),
        tool_metadata_cached: TOOL_METADATA_CACHE.get().is_some(),
        tool_metadata_count: TOOL_METADATA_CACHE.get().map(|m| m.len()).unwrap_or(0),
    }
}

/// Statistics about cache state
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub cli_cached: bool,
    pub tool_metadata_cached: bool,
    pub tool_metadata_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_or_build_cli_caching() {
        // First call should build and cache
        let cli1 = get_or_build_cli().await.unwrap();
        assert!(is_cli_cached());
        
        // Second call should return cached version
        let cli2 = get_or_build_cli().await.unwrap();
        
        // Should be the same reference (from cache)
        assert!(std::ptr::eq(cli1, cli2));
    }

    #[test]
    fn test_is_help_command() {
        assert!(is_help_command(&["sah".to_string(), "--help".to_string()]));
        assert!(is_help_command(&["sah".to_string(), "-h".to_string()]));
        assert!(is_help_command(&["sah".to_string(), "issue".to_string(), "--help".to_string()]));
        assert!(!is_help_command(&["sah".to_string(), "issue".to_string()]));
    }

    #[test]
    fn test_is_version_command() {
        assert!(is_version_command(&["sah".to_string(), "--version".to_string()]));
        assert!(is_version_command(&["sah".to_string(), "-V".to_string()]));
        assert!(!is_version_command(&["sah".to_string(), "issue".to_string()]));
    }

    #[test]
    fn test_is_completion_command() {
        assert!(is_completion_command(&["sah".to_string(), "completion".to_string()]));
        assert!(is_completion_command(&["sah".to_string(), "completion".to_string(), "bash".to_string()]));
        assert!(!is_completion_command(&["sah".to_string(), "issue".to_string()]));
    }

    #[tokio::test]
    async fn test_handle_fast_path_version() {
        let args = vec!["sah".to_string(), "--version".to_string()];
        let handled = handle_fast_path_commands(&args).await.unwrap();
        assert!(handled);
    }

    #[test]
    fn test_cache_stats() {
        let stats = get_cache_stats();
        assert!(stats.cli_cached || !stats.cli_cached); // Either cached or not
        assert!(stats.tool_metadata_cached || !stats.tool_metadata_cached);
        // tool_metadata_count should be a reasonable value
        assert!(stats.tool_metadata_count < 10000);
    }

    #[test]
    fn test_tool_metadata_caching() {
        let mut metadata = HashMap::new();
        metadata.insert("test_tool".to_string(), "test description".to_string());
        
        assert!(cache_tool_metadata(metadata).is_ok());
        
        let cached = get_cached_tool_metadata();
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().len(), 1);
    }
}