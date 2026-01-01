//! Configuration types for agent, queue, and session management.
//!
//! This module contains the main configuration structures used to set up
//! and configure agent behavior, request queuing, and session management.

use llama_loader::ModelConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use crate::types::errors::{AgentError, MCPError, QueueError, SessionError};
use crate::types::mcp::MCPServerConfig;
use crate::types::sessions::CompactionConfig;
use crate::types::tools::ParallelConfig;

/// Main configuration for an agent instance.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentConfig {
    pub model: ModelConfig,
    pub queue_config: QueueConfig,
    pub mcp_servers: Vec<MCPServerConfig>,
    pub session_config: SessionConfig,
    pub parallel_execution_config: ParallelConfig,
}

impl AgentConfig {
    pub fn validate(&self) -> Result<(), AgentError> {
        self.model.validate()?;
        self.queue_config.validate()?;
        self.session_config.validate()?;

        for server_config in &self.mcp_servers {
            server_config.validate()?;
        }

        // Check for duplicate MCP server names
        let mut server_names = HashSet::new();
        for server_config in &self.mcp_servers {
            if !server_names.insert(server_config.name()) {
                return Err(AgentError::MCP(MCPError::Protocol(format!(
                    "Duplicate MCP server name: {}",
                    server_config.name()
                ))));
            }
        }

        Ok(())
    }
}

/// Configuration for request queue management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueConfig {
    pub max_queue_size: usize,
    pub worker_threads: usize,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 100,
            worker_threads: 1,
        }
    }
}

impl QueueConfig {
    pub fn validate(&self) -> Result<(), QueueError> {
        if self.max_queue_size == 0 {
            return Err(QueueError::WorkerError(
                "Queue size must be greater than 0".to_string(),
            ));
        }

        if self.worker_threads == 0 {
            return Err(QueueError::WorkerError(
                "Worker threads must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }
}

/// Configuration for session management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub max_sessions: usize,
    pub auto_compaction: Option<CompactionConfig>,
    pub model_context_size: usize,
    /// Enable session persistence to disk
    pub persistence_enabled: bool,
    /// Directory for storing session files (defaults to .llama-sessions/)
    pub session_storage_dir: Option<PathBuf>,
    /// Time-to-live for session files in hours (0 = no cleanup)
    pub session_ttl_hours: u32,
    /// Auto-save after this many messages/tokens changed
    pub auto_save_threshold: usize,
    /// Maximum number of KV cache files to keep (LRU eviction, 0 = unlimited)
    pub max_kv_cache_files: usize,
    /// Directory for storing KV cache files (defaults to ~/.cache/llama-sessions/)
    /// Can be overridden with LLAMA_KV_CACHE_DIR environment variable
    pub kv_cache_dir: Option<PathBuf>,
}

impl Default for SessionConfig {
    fn default() -> Self {
        // Check for LLAMA_KV_CACHE_DIR env var, otherwise use default cache directory
        let kv_cache_dir = std::env::var("LLAMA_KV_CACHE_DIR")
            .ok()
            .map(PathBuf::from)
            .or_else(|| dirs::cache_dir().map(|cache| cache.join("llama-sessions")));

        Self {
            max_sessions: 1000,
            auto_compaction: None,
            model_context_size: 4096, // Standard context window size
            persistence_enabled: false,
            session_storage_dir: None, // Will use .llama-sessions/ by default
            session_ttl_hours: 24 * 7, // 1 week default
            auto_save_threshold: 5,    // Save after 5 messages
            max_kv_cache_files: 32,    // Keep 32 most recently used KV cache files
            kv_cache_dir,              // ~/.cache/llama-sessions/ or LLAMA_KV_CACHE_DIR
        }
    }
}

impl SessionConfig {
    pub fn validate(&self) -> Result<(), SessionError> {
        if self.max_sessions == 0 {
            return Err(SessionError::InvalidState(
                "Max sessions must be greater than 0".to_string(),
            ));
        }

        if self.persistence_enabled && self.auto_save_threshold == 0 {
            return Err(SessionError::InvalidState(
                "Auto save threshold must be greater than 0 when persistence is enabled"
                    .to_string(),
            ));
        }

        if let Some(ref dir) = self.session_storage_dir {
            if dir.as_os_str().is_empty() {
                return Err(SessionError::InvalidState(
                    "Session storage directory cannot be empty".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Get the KV cache directory for a specific model
    /// Returns base_dir/<model_hash>/
    pub fn get_model_kv_cache_dir(&self, model_config: &ModelConfig) -> PathBuf {
        let base_dir = self.kv_cache_dir.clone().unwrap_or_else(|| {
            dirs::cache_dir()
                .map(|cache| cache.join("llama-sessions"))
                .unwrap_or_else(|| PathBuf::from(".llama-sessions"))
        });

        let model_hash = model_config.compute_model_hash();
        base_dir.join(model_hash)
    }
}

/// Health status information for an agent.
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: String,
    pub model_loaded: bool,
    pub queue_size: usize,
    pub active_sessions: usize,
    pub uptime: Duration,
}
