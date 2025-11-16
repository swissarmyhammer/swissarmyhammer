//! Cache command implementation for managing rule evaluation cache

use crate::error::{CliError, CliResult};
use swissarmyhammer_rules::RuleCache;

use super::cli::{CacheAction, CacheCommand};

/// Execute cache command
pub async fn execute_cache_command(
    command: CacheCommand,
    context: &crate::context::CliContext,
) -> CliResult<()> {
    match command.action {
        CacheAction::Clear => clear_cache(context).await,
    }
}

/// Clear all cache entries
async fn clear_cache(context: &crate::context::CliContext) -> CliResult<()> {
    if context.test_mode {
        println!("Test mode: Cache clear skipped");
        return Ok(());
    }

    let cache = RuleCache::new()
        .map_err(|e| CliError::new(format!("Failed to initialize cache: {}", e), 1))?;

    let count = cache
        .clear()
        .map_err(|e| CliError::new(format!("Failed to clear cache: {}", e), 1))?;

    if count == 0 {
        println!("Cache is already empty");
    } else {
        println!("Cleared {} cache entries", count);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::CliContextBuilder;
    use swissarmyhammer_config::TemplateContext;

    async fn create_test_context() -> crate::context::CliContext {
        let template_context = TemplateContext::new();
        let matches = clap::Command::new("test")
            .try_get_matches_from(["test"])
            .unwrap();
        CliContextBuilder::default()
            .template_context(template_context)
            .format(crate::cli::OutputFormat::Table)
            .format_option(None)
            .verbose(false)
            .debug(false)
            .quiet(true)
            .test_mode(true)
            .matches(matches)
            .build_async()
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn test_clear_cache_empty() {
        use super::super::cli::{CacheAction, CacheCommand};

        let context = create_test_context().await;
        let command = CacheCommand {
            action: CacheAction::Clear,
        };

        let result = execute_cache_command(command, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_clear_cache_with_entries() {
        use swissarmyhammer_rules::{CachedResult, RuleCache, Severity};

        let context = create_test_context().await;

        // Add some cache entries
        let cache = RuleCache::new().unwrap();
        let key = RuleCache::calculate_cache_key("content", "rule", Severity::Error);
        cache.store(&key, &CachedResult::Pass).unwrap();

        // Clear the cache
        let command = CacheCommand {
            action: CacheAction::Clear,
        };

        let result = execute_cache_command(command, &context).await;
        assert!(result.is_ok());

        // Verify cache is empty
        let cache = RuleCache::new().unwrap();
        assert!(cache.get(&key).unwrap().is_none());
    }
}
