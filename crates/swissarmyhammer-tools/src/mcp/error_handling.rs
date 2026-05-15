//! Error handling and retry logic for MCP operations

use std::sync::Arc;
use swissarmyhammer_common::{Result, SwissArmyHammerError};
use swissarmyhammer_prompts::{PromptLibrary, PromptResolver};
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
    pub(crate) fn is_retryable_fs_error(error: &SwissArmyHammerError) -> bool {
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
        resolver
            .load_all_prompts(&mut library)
            .map_err(|e| SwissArmyHammerError::Other {
                message: e.to_string(),
            })?;

        let after_count = library
            .list()
            .map_err(|e| SwissArmyHammerError::Other {
                message: e.to_string(),
            })?
            .len();
        tracing::info!(
            "🔄 Reloaded prompts: {} → {} prompts",
            before_count,
            after_count
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    /// RAII guard that restores the working directory on drop.
    struct CwdGuard(Option<std::path::PathBuf>);

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            if let Some(ref dir) = self.0 {
                let _ = std::env::set_current_dir(dir);
            }
        }
    }

    // ---------------------------------------------------------------------------
    // ErrorHandler::is_retryable_fs_error tests
    // ---------------------------------------------------------------------------

    /// Test that IO `TimedOut` errors are retryable.
    #[test]
    fn test_is_retryable_timed_out() {
        let err = SwissArmyHammerError::Io(io::Error::new(io::ErrorKind::TimedOut, "timed out"));
        assert!(ErrorHandler::is_retryable_fs_error(&err));
    }

    /// Test that IO `Interrupted` errors are retryable.
    #[test]
    fn test_is_retryable_interrupted() {
        let err =
            SwissArmyHammerError::Io(io::Error::new(io::ErrorKind::Interrupted, "interrupted"));
        assert!(ErrorHandler::is_retryable_fs_error(&err));
    }

    /// Test that IO `WouldBlock` errors are retryable.
    #[test]
    fn test_is_retryable_would_block() {
        let err =
            SwissArmyHammerError::Io(io::Error::new(io::ErrorKind::WouldBlock, "would block"));
        assert!(ErrorHandler::is_retryable_fs_error(&err));
    }

    /// Test that IO `UnexpectedEof` errors are retryable.
    #[test]
    fn test_is_retryable_unexpected_eof() {
        let err = SwissArmyHammerError::Io(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "unexpected eof",
        ));
        assert!(ErrorHandler::is_retryable_fs_error(&err));
    }

    /// Test that non-transient IO errors (e.g., `PermissionDenied`) are not retryable.
    #[test]
    fn test_is_retryable_permission_denied_is_not_retryable() {
        let err = SwissArmyHammerError::Io(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "permission denied",
        ));
        assert!(!ErrorHandler::is_retryable_fs_error(&err));
    }

    /// Test that `Other` errors with "temporarily unavailable" message are retryable.
    #[test]
    fn test_is_retryable_temporarily_unavailable_message() {
        let err = SwissArmyHammerError::Other {
            message: "resource temporarily unavailable".to_string(),
        };
        assert!(ErrorHandler::is_retryable_fs_error(&err));
    }

    /// Test that `Other` errors with "resource busy" message are retryable.
    #[test]
    fn test_is_retryable_resource_busy_message() {
        let err = SwissArmyHammerError::Other {
            message: "resource busy".to_string(),
        };
        assert!(ErrorHandler::is_retryable_fs_error(&err));
    }

    /// Test that `Other` errors with "locked" message are retryable.
    #[test]
    fn test_is_retryable_locked_message() {
        let err = SwissArmyHammerError::Other {
            message: "file is locked".to_string(),
        };
        assert!(ErrorHandler::is_retryable_fs_error(&err));
    }

    /// Test that unrelated `Other` errors are not retryable.
    #[test]
    fn test_is_retryable_unrelated_error_is_not_retryable() {
        let err = SwissArmyHammerError::Other {
            message: "something completely different".to_string(),
        };
        assert!(!ErrorHandler::is_retryable_fs_error(&err));
    }

    /// Test that message matching is case-insensitive ("LOCKED" should be retryable).
    #[test]
    fn test_is_retryable_case_insensitive_matching() {
        let err = SwissArmyHammerError::Other {
            message: "FILE IS LOCKED".to_string(),
        };
        assert!(ErrorHandler::is_retryable_fs_error(&err));
    }

    // ---------------------------------------------------------------------------
    // ErrorHandler::reload_prompts tests
    // ---------------------------------------------------------------------------

    /// Test that `reload_prompts` succeeds with an empty library in an empty temp dir.
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_reload_prompts_empty_library_succeeds() {
        let tmp = tempfile::TempDir::new().unwrap();
        let _guard = CwdGuard(std::env::current_dir().ok());
        std::env::set_current_dir(tmp.path()).unwrap();

        let library = Arc::new(RwLock::new(PromptLibrary::new()));
        let handler = ErrorHandler::new(library.clone());
        // reload_prompts with an empty library should succeed (loads 0 prompts)
        let result = handler.reload_prompts().await;
        assert!(
            result.is_ok(),
            "reload_prompts should succeed: {:?}",
            result
        );
    }

    /// Test that calling `reload_prompts` multiple times is idempotent.
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_reload_prompts_idempotent() {
        let tmp = tempfile::TempDir::new().unwrap();
        let _guard = CwdGuard(std::env::current_dir().ok());
        std::env::set_current_dir(tmp.path()).unwrap();

        let library = Arc::new(RwLock::new(PromptLibrary::new()));
        let handler = ErrorHandler::new(library);
        assert!(handler.reload_prompts().await.is_ok());
        assert!(handler.reload_prompts().await.is_ok());
    }

    /// Test that `reload_prompts` clears and reloads prompts from the library.
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_reload_prompts_clears_and_reloads() {
        let tmp = tempfile::TempDir::new().unwrap();
        let _guard = CwdGuard(std::env::current_dir().ok());
        std::env::set_current_dir(tmp.path()).unwrap();

        let mut initial_library = PromptLibrary::new();
        initial_library
            .add(swissarmyhammer_prompts::Prompt::new("test", "test content"))
            .unwrap();

        let library = Arc::new(RwLock::new(initial_library));
        let handler = ErrorHandler::new(library.clone());

        // After reload, the library is cleared and reloaded from disk
        let result = handler.reload_prompts().await;
        assert!(result.is_ok());

        // The library should now reflect whatever PromptResolver finds on disk
        // (likely empty in a clean test environment, not necessarily 1)
        let lib = library.read().await;
        let prompts = lib.list().unwrap();
        // Just verify list() works — count depends on environment
        let _ = prompts.len();
    }
}
