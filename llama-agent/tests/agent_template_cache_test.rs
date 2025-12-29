//! Integration tests for Agent template cache workflow.
//!
//! These tests verify that the Agent properly integrates template caching
//! into its generation workflow, including lazy initialization, session
//! updates, and cache sharing across sessions.
//!
//! NOTE: These tests require a real model file to run and access to internal
//! APIs not exposed via the public AgentAPI trait. They are marked with
//! `#[ignore]` and serve as documentation of the expected behavior.
//!
//! The implementation in `llama-agent/src/agent.rs` ensures that:
//! 1. Template initialization happens lazily on first generation
//! 2. The session's `template_token_count` is set after initialization
//! 3. Subsequent generations use the cached template
//! 4. Multiple sessions with the same template share cache entries
//!
//! To verify this behavior, inspect the code at:
//! - `llama-agent/src/agent.rs:535-580` - Template initialization in Agent.generate()
//! - `llama-agent/src/model.rs:542-605` - ModelManager.initialize_session_with_template()
//! - `llama-agent/src/types/sessions.rs:241-269` - Session.template_token_count field

#[cfg(test)]
mod agent_template_cache_tests {
    use llama_agent::types::{
        AgentConfig, ModelConfig, ModelSource, ParallelConfig, QueueConfig, RetryConfig,
        SessionConfig,
    };
    use tempfile::TempDir;

    /// Helper to create a test agent configuration.
    fn _create_test_agent_config() -> AgentConfig {
        let temp_dir = TempDir::new().unwrap();

        AgentConfig {
            model: ModelConfig {
                source: ModelSource::Local {
                    folder: temp_dir.path().to_path_buf(),
                    filename: None,
                },
                batch_size: 512,
                n_seq_max: 2,
                n_threads: 1,
                n_threads_batch: 1,
                use_hf_params: false,
                retry_config: RetryConfig::default(),
                debug: false,
            },
            queue_config: QueueConfig::default(),
            session_config: SessionConfig::default(),
            mcp_servers: Vec::new(),
            parallel_execution_config: ParallelConfig::default(),
        }
    }

    /// Test that agent initializes template on first generation.
    ///
    /// **Expected Behavior:**
    /// - Session.template_token_count is None before first generation
    /// - Agent.generate() checks for None and calls initialize_session_with_template()
    /// - Session.template_token_count is set to Some(count) after first generation
    /// - Subsequent generations skip initialization
    ///
    /// **Verification:**
    /// This behavior is implemented in `llama-agent/src/agent.rs:535-580`
    #[test]
    #[ignore = "Requires real model and internal API access"]
    fn test_agent_initializes_template_on_first_generation_documentation() {
        // Implementation is in llama-agent/src/agent.rs:535-580
        // The generate() method checks if session.template_token_count.is_none()
        // and calls model_manager.initialize_session_with_template() if so
    }

    /// Test that multiple sessions with same template share cache.
    ///
    /// **Expected Behavior:**
    /// - First session processes template and saves to cache
    /// - Second session with same system prompt + tools loads from cache (cache hit)
    /// - Both sessions have the same template_token_count value
    /// - Cache stats show hits > 0
    ///
    /// **Verification:**
    /// Cache sharing is implemented in:
    /// - `llama-agent/src/model.rs:542-605` - initialize_session_with_template()
    /// - `llama-agent/src/template_cache.rs` - TemplateCache implementation
    #[test]
    #[ignore = "Requires real model and internal API access"]
    fn test_multiple_sessions_share_template_cache_documentation() {
        // Implementation uses template hash (system prompt + tools JSON) as cache key
        // Cache hit: loads KV cache from file
        // Cache miss: processes template and saves to file
    }

    /// Test that different templates create separate cache entries.
    ///
    /// **Expected Behavior:**
    /// - Sessions with different system prompts get different cache entries
    /// - Sessions with different tools get different cache entries
    /// - Each unique template results in a cache miss on first use
    /// - Cache stats show misses equal to number of unique templates
    ///
    /// **Verification:**
    /// Template hashing is implemented in `llama-agent/src/template_cache.rs`
    /// Hash is computed from system_prompt + tools_json for uniqueness
    #[test]
    #[ignore = "Requires real model and internal API access"]
    fn test_different_templates_use_different_cache_entries_documentation() {
        // Each unique (system_prompt, tools_json) combination gets a unique hash
        // Different hashes result in separate cache files
    }

    /// Compile-time verification that template_token_count field exists.
    ///
    /// This test ensures the Session struct has the template_token_count field
    /// with the correct type, which is required for template caching integration.
    #[test]
    fn test_session_has_template_token_count_field() {
        use llama_agent::types::{Message, MessageRole, Session, SessionId};
        use std::time::SystemTime;

        let session = Session {
            id: SessionId::new(),
            messages: vec![Message {
                role: MessageRole::System,
                content: "test".to_string(),
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
            #[cfg(feature = "acp")]
            todos: Vec::new(),
            #[cfg(feature = "acp")]
            available_commands: Vec::new(),
            current_mode: None,
            #[cfg(feature = "acp")]
            client_capabilities: None,
        };

        // Verify field exists and has correct type
        let _: Option<usize> = session.template_token_count;
        assert!(session.template_token_count.is_none());
    }
}
