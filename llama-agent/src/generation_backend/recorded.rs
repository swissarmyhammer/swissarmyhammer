//! Recorded generation backend for fixture playback

use super::GenerationBackend;
use crate::types::{
    AgentError, FinishReason, GenerationRequest, GenerationResponse, Session, StreamChunk,
};
use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::pin::Pin;
use std::sync::Mutex;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

/// Stream chunk data for recording/playback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunkData {
    pub text: String,
    pub is_complete: bool,
    pub token_count: u32,
    pub finish_reason: Option<FinishReason>,
}

impl From<StreamChunk> for StreamChunkData {
    fn from(chunk: StreamChunk) -> Self {
        Self {
            text: chunk.text,
            is_complete: chunk.is_complete,
            token_count: chunk.token_count,
            finish_reason: chunk.finish_reason,
        }
    }
}

impl StreamChunkData {
    pub fn to_stream_chunk(&self) -> StreamChunk {
        StreamChunk {
            text: self.text.clone(),
            is_complete: self.is_complete,
            token_count: self.token_count,
            finish_reason: self.finish_reason.clone(),
        }
    }
}

/// Generation response data for recording/playback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationResponseData {
    pub generated_text: String,
    pub tokens_generated: u32,
    pub finish_reason: FinishReason,
    /// Generation time in milliseconds
    #[serde(default)]
    pub generation_time_ms: u64,
}

impl From<GenerationResponse> for GenerationResponseData {
    fn from(response: GenerationResponse) -> Self {
        Self {
            generated_text: response.generated_text,
            tokens_generated: response.tokens_generated,
            finish_reason: response.finish_reason,
            generation_time_ms: response.generation_time.as_millis() as u64,
        }
    }
}

impl GenerationResponseData {
    pub fn to_generation_response(&self) -> GenerationResponse {
        GenerationResponse {
            generated_text: self.generated_text.clone(),
            tokens_generated: self.tokens_generated,
            generation_time: std::time::Duration::from_millis(self.generation_time_ms),
            finish_reason: self.finish_reason.clone(),
            complete_token_sequence: None, // Not stored in playback fixtures
        }
    }
}

/// A single request/response exchange
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationExchange {
    /// The input prompt (for matching/verification)
    pub prompt: String,
    /// Session ID for this exchange
    pub session_id: String,
    /// Streaming response chunks (in order)
    pub chunks: Vec<StreamChunkData>,
    /// Final response after streaming completes
    pub response: GenerationResponseData,
}

/// Recorded generation session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedSession {
    /// Sequential exchanges (request -> chunks -> response)
    pub exchanges: Vec<GenerationExchange>,
}

/// Backend that plays back from a recorded fixture
///
/// This implementation reads from a pre-recorded session fixture,
/// allowing tests to run without loading models or performing inference.
pub struct RecordedGenerationBackend {
    /// The recorded session data
    session: RecordedSession,
    /// Current exchange index (interior mutability for &self methods)
    exchange_idx: Mutex<usize>,
}

impl RecordedGenerationBackend {
    /// Create a new recorded backend from a fixture
    pub fn new(session: RecordedSession) -> Self {
        Self {
            session,
            exchange_idx: Mutex::new(0),
        }
    }

    /// Load a recorded session from a JSON file
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, AgentError> {
        let content = std::fs::read_to_string(path.as_ref()).map_err(|e| {
            AgentError::Session(crate::types::SessionError::InvalidState(format!(
                "Failed to read fixture at {:?}: {}",
                path.as_ref(),
                e
            )))
        })?;
        let session: RecordedSession = serde_json::from_str(&content).map_err(|e| {
            AgentError::Session(crate::types::SessionError::InvalidState(format!(
                "Failed to parse fixture JSON at {:?}: {}",
                path.as_ref(),
                e
            )))
        })?;
        Ok(Self::new(session))
    }

    /// Get the next exchange or error if exhausted
    fn get_next_exchange(&self) -> Result<GenerationExchange, AgentError> {
        let mut idx = self.exchange_idx.lock().unwrap();

        if *idx >= self.session.exchanges.len() {
            return Err(AgentError::Session(
                crate::types::SessionError::InvalidState(format!(
                    "Recorded session exhausted: attempted exchange {} but only {} recorded",
                    *idx + 1,
                    self.session.exchanges.len()
                )),
            ));
        }

        let exchange = self.session.exchanges[*idx].clone();
        *idx += 1;
        Ok(exchange)
    }
}

#[async_trait]
impl GenerationBackend for RecordedGenerationBackend {
    async fn generate(
        &self,
        _request: &GenerationRequest,
        _session: &Session,
    ) -> Result<GenerationResponse, AgentError> {
        // For non-streaming, return the final response from the next exchange
        let exchange = self.get_next_exchange()?;

        tracing::debug!(
            "RecordedBackend: Returning recorded response for session {}",
            exchange.session_id
        );

        Ok(exchange.response.to_generation_response())
    }

    async fn generate_stream(
        &self,
        _request: &GenerationRequest,
        _session: &Session,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, AgentError>> + Send>>, AgentError>
    {
        // Get the next exchange
        let exchange = self.get_next_exchange()?;

        tracing::debug!(
            "RecordedBackend: Streaming {} recorded chunks for session {}",
            exchange.chunks.len(),
            exchange.session_id
        );

        // Create a channel to emit chunks
        let (tx, rx) = mpsc::channel(100);

        // Spawn a task to emit all chunks
        let chunks = exchange.chunks.clone();
        tokio::spawn(async move {
            for chunk_data in chunks {
                let chunk = chunk_data.to_stream_chunk();
                if tx.send(Ok(chunk)).await.is_err() {
                    break; // Receiver dropped
                }
            }
        });

        Ok(Box::pin(ReceiverStream::new(rx)))
    }
}
