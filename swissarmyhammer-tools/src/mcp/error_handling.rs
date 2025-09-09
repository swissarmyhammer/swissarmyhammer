//! Error handling and retry logic for MCP operations

use std::sync::Arc;
use swissarmyhammer::{PromptLibrary, PromptResolver};
use swissarmyhammer_common::{SwissArmyHammerError, Result};
use tokio::sync::RwLock;

/// Error handling implementation for MCP server
pub struct ErrorHandler {
    library: Arc<RwLock<PromptLibrary>>,
}

impl ErrorHandler {
    /// Create a new error handler with the given prompt library
    pub fn new(library: Arc<RwLock<PromptLibrary>>) -> Self {
        Self { library }
    }

    /// Reload prompts from disk with retry logic.
    ///
    /// This method reloads all prompts from the file system and updates
    /// the internal library. It includes retry logic for transient errors.
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Ok if reload succeeds, error otherwise
    pub async fn reload_prompts(&self) -> Result<()> {
        self.reload_prompts_with_retry().await
    }

    /// Reload prompts with retry logic for transient file system errors
    async fn reload_prompts_with_retry(&self) -> Result<()> {
        const MAX_RETRIES: u32 = 3;
        const INITIAL_BACKOFF_MS: u64 = 100;

        let mut last_error = None;
        let mut backoff_ms = INITIAL_BACKOFF_MS;

        for attempt in 1..=MAX_RETRIES {
            match self.reload_prompts_internal().await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    last_error = Some(e);

                    // Check if this is a retryable error
                    if attempt < MAX_RETRIES
                        && Self::is_retryable_fs_error(last_error.as_ref().unwrap())
                    {
                        tracing::warn!(
                            "⚠️ Reload attempt {} failed, retrying in {}ms: {}",
                            attempt,
                            backoff_ms,
                            last_error.as_ref().unwrap()
                        );

                        tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
                        backoff_ms *= 2; // Exponential backoff
                    } else {
                        break;
                    }
                }
            }
        }

        Err(last_error.unwrap())
    }

    /// Check if an error is a retryable file system error
    fn is_retryable_fs_error(error: &SwissArmyHammerError) -> bool {
        // Check for common transient file system errors
        if let SwissArmyHammerError::Io(io_err) = error {
            matches!(
                io_err.kind(),
                std::io::ErrorKind::TimedOut
                    | std::io::ErrorKind::Interrupted
                    | std::io::ErrorKind::WouldBlock
                    | std::io::ErrorKind::UnexpectedEof
            )
        } else {
            // Also retry if the error message contains certain patterns
            let error_str = error.to_string().to_lowercase();
            error_str.contains("temporarily unavailable")
                || error_str.contains("resource busy")
                || error_str.contains("locked")
        }
    }

    /// Internal reload method that performs the actual reload
    async fn reload_prompts_internal(&self) -> Result<()> {
        let mut library = self.library.write().await;
        let mut resolver = PromptResolver::new();

        // Get count before reload (default to 0 if library.list() fails)
        let before_count = library.list().map(|p| p.len()).unwrap_or(0);

        // Clear existing prompts and reload
        *library = PromptLibrary::new();
        resolver.load_all_prompts(&mut library).map_err(|e| SwissArmyHammerError::Other { message: e.to_string() })?;

        let after_count = library.list().map_err(|e| SwissArmyHammerError::Other { message: e.to_string() })?.len();
        tracing::info!(
            "🔄 Reloaded prompts: {} → {} prompts",
            before_count,
            after_count
        );

        Ok(())
    }
}
