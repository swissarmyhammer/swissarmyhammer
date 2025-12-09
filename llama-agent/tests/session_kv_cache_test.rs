//! Integration tests for session KV cache persistence.
//!
//! These tests verify that conversation history is cached between prompts,
//! eliminating redundant token processing for multi-turn conversations.
//!
//! NOTE: These tests require a real model file to run and access to internal
//! APIs not exposed via the public AgentAPI trait. They are marked with
//! `#[ignore]` and serve as documentation of the expected behavior.
//!
//! The implementation should ensure that:
//! 1. Session KV cache is saved after each generation
//! 2. Session KV cache is loaded before generation (if it exists)
//! 3. Only new tokens are processed when cache is loaded
//! 4. Session KV cache is deleted when session is deleted
//!
//! To implement this behavior, changes are needed in:
//! - `llama-agent/src/queue.rs` - Add KV cache load/save around generation
//! - `llama-agent/src/types/generation.rs` - Add `complete_token_sequence` field
//! - `llama-agent/src/generation/mod.rs` - Track complete token sequences
//! - `llama-agent/src/session.rs` - Delete KV cache on session deletion

#[cfg(test)]
mod session_kv_cache_tests {
    use llama_agent::types::{
        AgentConfig, Message, MessageRole, ModelConfig, ModelSource, ParallelConfig, QueueConfig,
        RetryConfig, Session, SessionConfig, SessionId,
    };
    use std::time::SystemTime;
    use tempfile::TempDir;

    /// Helper to create a test agent configuration with session storage.
    fn _create_test_config_with_session_storage() -> (AgentConfig, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let session_dir = temp_dir.path().join(".llama-sessions");

        let config = AgentConfig {
            model: ModelConfig {
                source: ModelSource::Local {
                    folder: temp_dir.path().to_path_buf(),
                    filename: Some("test-model.gguf".to_string()),
                },
                batch_size: 512,
                n_seq_max: 8,
                n_threads: 1,
                n_threads_batch: 1,
                use_hf_params: false,
                retry_config: RetryConfig::default(),
                debug: false,
            },
            queue_config: QueueConfig {
                max_queue_size: 10,
                worker_threads: 1,
            },
            session_config: SessionConfig {
                persistence_enabled: true,
                session_storage_dir: Some(session_dir),
                ..Default::default()
            },
            mcp_servers: Vec::new(),
            parallel_execution_config: ParallelConfig::default(),
        };

        (config, temp_dir)
    }

    /// Helper to create a test session.
    fn _create_test_session() -> Session {
        Session {
            id: SessionId::new(),
            messages: vec![Message {
                role: MessageRole::User,
                content: "What is 2 + 2?".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            }],
            mcp_servers: Vec::new(),
            available_tools: Vec::new(),
            available_prompts: Vec::new(),
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
            compaction_history: Vec::new(),
            transcript_path: None,
            context_state: None,
            template_token_count: None,
        }
    }

    /// Test that session KV cache persists conversation history between prompts.
    ///
    /// **Expected Behavior:**
    /// - First prompt: No cache exists, process all tokens, save KV cache
    /// - Second prompt: Load cache, process only new tokens, save updated cache
    /// - Third prompt: Load cache, process only new tokens, save updated cache
    /// - Each subsequent prompt is fast (not reprocessing entire history)
    ///
    /// **Performance Impact:**
    /// ```
    /// Turn 1:  Process 50 new tokens
    /// Turn 2:  Process 200 tokens (without cache) → 50 tokens (with cache)
    /// Turn 3:  Process 450 tokens (without cache) → 50 tokens (with cache)
    /// Turn 10: Process 2500+ tokens (without cache) → 50 tokens (with cache)
    /// ```
    ///
    /// **Speedup:** O(n²) → O(k) where n = conversation length, k = single message length
    ///
    /// **Implementation:**
    /// In `llama-agent/src/queue.rs`, before generation:
    /// ```rust,ignore
    /// let session_offset = if model_manager.has_session_kv_cache(&session.id, &kv_cache_dir) {
    ///     match model_manager.load_session_kv_cache(&mut ctx, &session.id, &kv_cache_dir, ctx.n_ctx() as usize) {
    ///         Ok(cached_tokens) => Some(cached_tokens.len()),
    ///         Err(e) => {
    ///             warn!("Failed to load session KV cache: {}", e);
    ///             None
    ///         }
    ///     }
    /// } else {
    ///     None
    /// };
    /// ```
    ///
    /// After generation:
    /// ```rust,ignore
    /// if let Err(e) = model_manager.save_session_kv_cache(&ctx, &session.id, &all_tokens, &kv_cache_dir) {
    ///     warn!("Failed to save session KV cache: {}", e);
    /// }
    /// ```
    #[test]
    #[ignore = "Requires implementation and real model file"]
    fn test_session_kv_cache_persistence_documentation() {
        // This test would verify:
        // 1. KV cache file is created after first generation: {session_id}_kv.bin
        // 2. Subsequent generations are faster due to cached history
        // 3. Only new tokens are processed, not entire conversation history

        let (_config, _temp_dir) = _create_test_config_with_session_storage();
        let _session = _create_test_session();

        // Implementation needed in queue.rs to:
        // - Load KV cache before generation
        // - Save KV cache after generation
        // - Track complete token sequence in GenerationResponse
    }

    /// Test that session KV cache is deleted when session is deleted.
    ///
    /// **Expected Behavior:**
    /// - Create session and generate response (creates KV cache file)
    /// - Verify KV cache file exists: `{session_id}_kv.bin`
    /// - Delete session
    /// - Verify KV cache file is also deleted
    ///
    /// **Implementation:**
    /// In `llama-agent/src/session.rs`, add to session deletion:
    /// ```rust,ignore
    /// pub fn delete_session(&self, session_id: &SessionId) -> Result<(), SessionError> {
    ///     // Existing session file deletion
    ///     
    ///     // NEW: Also delete KV cache
    ///     let kv_cache_dir = self.get_cache_dir();
    ///     let _ = self.model_manager.delete_session_kv_cache(session_id, &kv_cache_dir);
    ///     
    ///     Ok(())
    /// }
    /// ```
    #[test]
    #[ignore = "Requires implementation and real model file"]
    fn test_session_kv_cache_cleanup_documentation() {
        // This test would verify:
        // 1. KV cache file is created during generation
        // 2. KV cache file is deleted when session is deleted
        // 3. No orphaned cache files remain after session cleanup

        let (_config, _temp_dir) = _create_test_config_with_session_storage();
        let _session = _create_test_session();

        // Implementation needed in session.rs to:
        // - Delete KV cache file on session deletion
    }

    /// Test interaction between template caching and session KV caching.
    ///
    /// **Expected Behavior:**
    /// - Template cache loads system prompt + tools (shared across sessions)
    /// - Session cache loads complete conversation (per-session)
    /// - Processing offset prioritizes session cache over template cache
    ///
    /// **Processing Logic:**
    /// ```rust,ignore
    /// if let Some(session_offset) = session_kv_loaded_offset {
    ///     // Session cache loaded - includes template + conversation
    ///     process_from_position(session_offset);
    /// } else if let Some(template_count) = session.template_token_count {
    ///     // No session cache, but template is cached
    ///     process_from_position(template_count);
    /// } else {
    ///     // No caching - process everything
    ///     process_from_position(0);
    /// }
    /// ```
    #[test]
    #[ignore = "Requires implementation and real model file"]
    fn test_template_and_session_cache_interaction_documentation() {
        // This test would verify:
        // 1. Template cache provides base offset for system prompt + tools
        // 2. Session cache provides full conversation offset
        // 3. Session cache supersedes template cache when present
        // 4. Proper fallback when session cache is unavailable

        let (_config, _temp_dir) = _create_test_config_with_session_storage();
        let _session = _create_test_session();

        // Implementation needed in queue.rs to:
        // - Properly prioritize session offset vs template offset
        // - Pass correct offset to generation helper
    }

    /// Compile-time verification that GenerationRequest fields exist.
    ///
    /// This test ensures the GenerationRequest struct has the correct
    /// constructor and builder methods for creating requests.
    #[test]
    fn test_generation_request_construction() {
        use llama_agent::types::{GenerationRequest, SessionId};

        let session_id = SessionId::new();

        // Test basic construction
        let request = GenerationRequest::new(session_id)
            .with_max_tokens(100)
            .with_temperature(0.1);

        assert_eq!(request.max_tokens, Some(100));
        assert_eq!(request.temperature, Some(0.1));
    }

    /// Compile-time verification that SessionConfig has session_storage_dir field.
    #[test]
    fn test_session_config_has_storage_dir() {
        use llama_agent::types::SessionConfig;
        use std::path::PathBuf;

        let config = SessionConfig {
            persistence_enabled: true,
            session_storage_dir: Some(PathBuf::from(".llama-sessions")),
            ..Default::default()
        };

        assert!(config.persistence_enabled);
        assert!(config.session_storage_dir.is_some());
    }

    /// Compile-time verification that SessionId::new() works correctly.
    #[test]
    fn test_session_id_construction() {
        use llama_agent::types::SessionId;

        let id1 = SessionId::new();
        let id2 = SessionId::new();

        // Each call should create a unique ID
        assert_ne!(format!("{:?}", id1), format!("{:?}", id2));
    }
}
