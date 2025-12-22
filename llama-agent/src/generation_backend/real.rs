//! Real generation backend using actual model inference

use super::GenerationBackend;
use crate::queue::RequestQueue;
use crate::session::SessionManager;
use crate::types::{AgentError, GenerationRequest, GenerationResponse, Session, StreamChunk};
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use std::pin::Pin;
use std::sync::Arc;
use tokio_stream::wrappers::ReceiverStream;

/// Backend that performs real model inference
///
/// This implementation delegates to the existing RequestQueue for actual
/// LLM generation using llama.cpp bindings.
pub struct RealGenerationBackend {
    request_queue: Arc<RequestQueue>,
    session_manager: Arc<SessionManager>,
}

impl RealGenerationBackend {
    /// Create a new real generation backend
    pub fn new(request_queue: Arc<RequestQueue>, session_manager: Arc<SessionManager>) -> Self {
        Self {
            request_queue,
            session_manager,
        }
    }
}

#[async_trait]
impl GenerationBackend for RealGenerationBackend {
    async fn generate(
        &self,
        request: &GenerationRequest,
        session: &Session,
    ) -> Result<GenerationResponse, AgentError> {
        // Delegate to the existing RequestQueue
        self.request_queue
            .submit_request(request.clone(), session)
            .await
            .map_err(AgentError::from)
    }

    async fn generate_stream(
        &self,
        request: &GenerationRequest,
        session: &Session,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, AgentError>> + Send>>, AgentError>
    {
        // Delegate to the existing RequestQueue
        let receiver = self
            .request_queue
            .submit_streaming_request(request.clone(), session)
            .await
            .map_err(AgentError::from)?;

        // Convert Receiver to Stream and map QueueError to AgentError
        let stream = ReceiverStream::new(receiver).map(
            |result: Result<StreamChunk, crate::types::QueueError>| {
                result.map_err(AgentError::from)
            },
        );

        Ok(Box::pin(stream))
    }
}
