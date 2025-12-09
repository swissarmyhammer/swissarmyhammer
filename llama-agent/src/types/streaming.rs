//! Streaming and async types for agent interactions.
//!
//! This module contains types for streaming responses and the main AgentAPI trait.

use async_trait::async_trait;
use futures::Stream;
// Note: serde::{Deserialize, Serialize} removed as unused
use std::pin::Pin;

use crate::types::configs::{AgentConfig, HealthStatus};
use crate::types::errors::AgentError;
use crate::types::generation::{GenerationRequest, GenerationResponse};
use crate::types::ids::SessionId;
use crate::types::messages::Message;
use crate::types::tools::{ToolCall, ToolResult};

/// A chunk of streaming text response.
#[derive(Debug)]
pub struct StreamChunk {
    pub text: String,
    pub is_complete: bool,
    pub token_count: u32,
}

/// Main agent API trait for implementing agent functionality.
///
/// Note: Some method signatures are simplified until all dependent types are moved.
#[async_trait]
pub trait AgentAPI {
    async fn initialize(config: AgentConfig) -> Result<Self, AgentError>
    where
        Self: Sized;

    async fn generate(&self, request: GenerationRequest) -> Result<GenerationResponse, AgentError>;

    async fn generate_stream(
        &self,
        request: GenerationRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, AgentError>> + Send>>, AgentError>;

    async fn create_session(&self) -> Result<crate::types::sessions::Session, AgentError>;

    async fn create_session_with_transcript(
        &self,
        transcript_path: Option<std::path::PathBuf>,
    ) -> Result<crate::types::sessions::Session, AgentError>;

    async fn get_session(
        &self,
        session_id: &SessionId,
    ) -> Result<Option<crate::types::sessions::Session>, AgentError>;

    async fn add_message(&self, session_id: &SessionId, message: Message)
        -> Result<(), AgentError>;

    async fn discover_tools(
        &self,
        session: &mut crate::types::sessions::Session,
    ) -> Result<(), AgentError>;

    async fn execute_tool(
        &self,
        tool_call: ToolCall,
        session: &crate::types::sessions::Session,
    ) -> Result<ToolResult, AgentError>;

    async fn health(&self) -> Result<HealthStatus, AgentError>;

    /// Compact a session using AI summarization
    async fn compact_session(
        &self,
        session_id: &SessionId,
        config: Option<crate::types::sessions::CompactionConfig>,
    ) -> Result<crate::session::CompactionResult, AgentError>;

    /// Check if a session should be compacted based on configuration
    async fn should_compact_session(
        &self,
        session_id: &SessionId,
        config: &crate::types::sessions::CompactionConfig,
    ) -> Result<bool, AgentError>;

    /// Auto-compact sessions that meet the compaction criteria
    async fn auto_compact_sessions(
        &self,
        config: &crate::types::sessions::CompactionConfig,
    ) -> Result<crate::session::CompactionSummary, AgentError>;
}
