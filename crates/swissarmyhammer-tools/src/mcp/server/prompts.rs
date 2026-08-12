//! Reloading the prompt library, and deciding whether the reload changed it.
//!
//! A file-system event says a prompt directory changed, not that any prompt
//! did. [`McpServer::reload_prompts`] therefore compares a content signature
//! taken before the reload against one taken after, and reports a change only
//! when the two differ — so a touched file raises no client notification.

use super::retry::retry_with_backoff;
use super::McpServer;
use std::collections::BTreeMap;

use swissarmyhammer_common::{Result, SwissArmyHammerError};
use swissarmyhammer_templating::{PromptResolver, TemplateLibrary};

impl McpServer {
    /// Compute a content signature for a set of prompts.
    ///
    /// This creates a deterministic snapshot of prompt content by serializing
    /// all relevant fields (excluding metadata like source_path) to JSON.
    /// Used to detect actual content changes vs. just file modification events.
    ///
    /// # Arguments
    ///
    /// * `prompts` - The prompts to compute signature for
    ///
    /// # Returns
    ///
    /// A BTreeMap where keys are prompt names and values are JSON representations
    /// of the prompt content (excluding source_path)
    fn compute_prompt_signature(
        prompts: &[swissarmyhammer_templating::Prompt],
    ) -> BTreeMap<String, String> {
        let mut signature = BTreeMap::new();
        for prompt in prompts {
            // Create a simplified representation without source_path
            let content = serde_json::json!({
                "name": prompt.name,
                "description": prompt.description,
                "category": prompt.category,
                "tags": prompt.tags,
                "template": prompt.template,
                "parameters": prompt.parameters,
            });
            // Use compact JSON representation for comparison
            if let Ok(json_str) = serde_json::to_string(&content) {
                signature.insert(prompt.name.clone(), json_str);
            }
        }
        signature
    }

    /// Reload prompts from disk with retry logic.
    ///
    /// This method reloads all prompts from the file system and updates
    /// the internal library. It includes retry logic for transient errors.
    ///
    /// # Returns
    ///
    /// * `Result<bool>` - Ok(true) if prompts changed, Ok(false) if no changes, error otherwise
    ///
    /// # Errors
    ///
    /// Returns error if prompt directories cannot be read or prompts cannot be loaded
    pub async fn reload_prompts(&self) -> Result<bool> {
        self.reload_prompts_with_retry().await
    }

    /// Reload prompts with retry logic for transient file system errors
    async fn reload_prompts_with_retry(&self) -> Result<bool> {
        retry_with_backoff(
            || self.reload_prompts_internal(),
            Self::is_retryable_fs_error,
            "Reload",
        )
        .await
    }

    /// Check if an error is a retryable file system error
    pub(super) fn is_retryable_fs_error(error: &SwissArmyHammerError) -> bool {
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
    ///
    /// # Returns
    ///
    /// * `Result<bool>` - Ok(true) if prompt content changed, Ok(false) if no changes
    async fn reload_prompts_internal(&self) -> Result<bool> {
        let mut library = self.library.write().await;
        let mut resolver = PromptResolver::new();

        // Capture "before" state
        let before_prompts = library.list().unwrap_or_default();
        let before_count = before_prompts.len();
        let before_signature = Self::compute_prompt_signature(&before_prompts);

        // Clear existing prompts and reload
        *library = TemplateLibrary::new();
        resolver
            .load_all_prompts(&mut library)
            .map_err(|e| SwissArmyHammerError::Other {
                message: e.to_string(),
            })?;

        // Capture "after" state
        let after_prompts = library.list().map_err(|e| SwissArmyHammerError::Other {
            message: e.to_string(),
        })?;
        let after_count = after_prompts.len();
        let after_signature = Self::compute_prompt_signature(&after_prompts);

        // Compare signatures to detect actual content changes
        let has_changes = before_signature != after_signature;

        tracing::info!(
            "🔄 Reloaded prompts: {} → {} prompts{}",
            before_count,
            after_count,
            if has_changes {
                " (content changed)"
            } else {
                " (no content changes)"
            }
        );

        Ok(has_changes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------
    // reload_prompts() tests
    // ---------------------------------------------------------------

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_reload_prompts_succeeds() {
        let server = McpServer::new(TemplateLibrary::default()).await.unwrap();
        // Initialize first to load prompts
        server.initialize().await.unwrap();

        // Reload should succeed
        let result = server.reload_prompts().await;
        assert!(
            result.is_ok(),
            "reload_prompts should succeed: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_reload_prompts_detects_no_change() {
        let server = McpServer::new(TemplateLibrary::default()).await.unwrap();
        server.initialize().await.unwrap();

        // Reloading without changes should return false (no content change)
        let changed = server.reload_prompts().await.unwrap();
        assert!(
            !changed,
            "Reloading without filesystem changes should report no change"
        );
    }

    #[test]
    fn test_is_retryable_fs_error_io_kinds() {
        let retryable_kinds = [
            std::io::ErrorKind::TimedOut,
            std::io::ErrorKind::Interrupted,
            std::io::ErrorKind::WouldBlock,
            std::io::ErrorKind::UnexpectedEof,
        ];
        for kind in retryable_kinds {
            let err = SwissArmyHammerError::Io(std::io::Error::new(kind, "test"));
            assert!(
                McpServer::is_retryable_fs_error(&err),
                "{:?} should be retryable",
                kind
            );
        }

        let non_retryable = SwissArmyHammerError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "not found",
        ));
        assert!(!McpServer::is_retryable_fs_error(&non_retryable));
    }

    #[test]
    fn test_is_retryable_fs_error_message_patterns() {
        let err = SwissArmyHammerError::Other {
            message: "resource temporarily unavailable".to_string(),
        };
        assert!(McpServer::is_retryable_fs_error(&err));

        let err = SwissArmyHammerError::Other {
            message: "file is locked by another process".to_string(),
        };
        assert!(McpServer::is_retryable_fs_error(&err));

        let err = SwissArmyHammerError::Other {
            message: "resource busy, try again".to_string(),
        };
        assert!(McpServer::is_retryable_fs_error(&err));
    }

    // ---------------------------------------------------------------
    // compute_prompt_signature() tests
    // ---------------------------------------------------------------

    #[test]
    fn test_compute_prompt_signature_deterministic() {
        use swissarmyhammer_templating::Prompt;
        let prompts = vec![Prompt::new("a", "Hello"), Prompt::new("b", "World")];
        let sig1 = McpServer::compute_prompt_signature(&prompts);
        let sig2 = McpServer::compute_prompt_signature(&prompts);
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn test_compute_prompt_signature_detects_changes() {
        use swissarmyhammer_templating::Prompt;
        let prompts_v1 = vec![Prompt::new("a", "Hello")];
        let prompts_v2 = vec![Prompt::new("a", "Hello updated")];
        let sig1 = McpServer::compute_prompt_signature(&prompts_v1);
        let sig2 = McpServer::compute_prompt_signature(&prompts_v2);
        assert_ne!(
            sig1, sig2,
            "Different content should produce different signatures"
        );
    }
}
