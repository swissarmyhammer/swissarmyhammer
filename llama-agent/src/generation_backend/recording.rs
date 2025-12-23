//! Recording generation backend for fixture capture

use super::{GenerationBackend, RealGenerationBackend};
use crate::generation_backend::recorded::{
    GenerationExchange, GenerationResponseData, RecordedSession, StreamChunkData,
};
use crate::types::{AgentError, GenerationRequest, GenerationResponse, Session, StreamChunk};
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

/// Backend that records generation to a JSON fixture
///
/// This wraps a real backend and captures all exchanges to a file on drop.
pub struct RecordingGenerationBackend {
    /// Real backend for actual inference
    real_backend: Arc<RealGenerationBackend>,
    /// Output path for the recording
    output_path: PathBuf,
    /// Recorded exchanges
    exchanges: Arc<Mutex<Vec<GenerationExchange>>>,
}

impl RecordingGenerationBackend {
    /// Create a new recording backend
    pub fn new(
        real_backend: Arc<RealGenerationBackend>,
        output_path: PathBuf,
    ) -> Self {
        tracing::info!("RecordingBackend: Will record to {:?}", output_path);
        Self {
            real_backend,
            output_path,
            exchanges: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Save recorded exchanges to file
    fn save_recording(&self) -> Result<(), AgentError> {
        let exchanges = self.exchanges.lock().unwrap();
        let session = RecordedSession {
            exchanges: exchanges.clone(),
        };

        // Ensure parent directory exists
        if let Some(parent) = self.output_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AgentError::Session(crate::types::SessionError::InvalidState(format!(
                    "Failed to create fixture directory: {}",
                    e
                )))
            })?;
        }

        let json = serde_json::to_string_pretty(&session).map_err(|e| {
            AgentError::Session(crate::types::SessionError::InvalidState(format!(
                "Failed to serialize recording: {}",
                e
            )))
        })?;

        std::fs::write(&self.output_path, json).map_err(|e| {
            AgentError::Session(crate::types::SessionError::InvalidState(format!(
                "Failed to write recording to {:?}: {}",
                self.output_path, e
            )))
        })?;

        tracing::info!(
            "RecordingBackend: Saved {} exchanges to {:?}",
            exchanges.len(),
            self.output_path
        );
        Ok(())
    }
}

impl Drop for RecordingGenerationBackend {
    fn drop(&mut self) {
        if let Err(e) = self.save_recording() {
            tracing::error!("Failed to save recording on drop: {}", e);
        }
    }
}

#[async_trait]
impl GenerationBackend for RecordingGenerationBackend {
    async fn generate(
        &self,
        request: &GenerationRequest,
        session: &Session,
    ) -> Result<GenerationResponse, AgentError> {
        // Perform real generation
        let response = self.real_backend.generate(request, session).await?;

        // Record the exchange
        let exchange = GenerationExchange {
            prompt: format!("Request for session {}", request.session_id),
            session_id: request.session_id.to_string(),
            chunks: vec![StreamChunkData {
                text: response.generated_text.clone(),
                is_complete: true,
                token_count: response.tokens_generated,
                finish_reason: Some(response.finish_reason.clone()),
            }],
            response: GenerationResponseData::from(response.clone()),
        };

        self.exchanges.lock().unwrap().push(exchange);

        Ok(response)
    }

    async fn generate_stream(
        &self,
        request: &GenerationRequest,
        session: &Session,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, AgentError>> + Send>>, AgentError>
    {
        // Get the real stream
        let mut real_stream = self.real_backend.generate_stream(request, session).await?;

        // Create a channel to forward chunks while recording
        let (tx, rx) = mpsc::channel(100);

        let exchanges = Arc::clone(&self.exchanges);
        let session_id = request.session_id.to_string();
        let prompt = format!("Request for session {}", request.session_id);

        // Spawn task to consume real stream and record
        tokio::spawn(async move {
            let mut recorded_chunks = Vec::new();
            let mut accumulated_text = String::new();
            let mut total_tokens = 0;
            let mut last_finish_reason = None;

            while let Some(chunk_result) = real_stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        // Record chunk data
                        recorded_chunks.push(StreamChunkData::from(chunk.clone()));
                        accumulated_text.push_str(&chunk.text);
                        total_tokens += chunk.token_count;
                        if let Some(reason) = &chunk.finish_reason {
                            last_finish_reason = Some(reason.clone());
                        }

                        // Forward to test
                        if tx.send(Ok(chunk)).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e)).await;
                        break;
                    }
                }
            }

            // Save exchange
            let exchange = GenerationExchange {
                prompt,
                session_id,
                chunks: recorded_chunks,
                response: GenerationResponseData {
                    generated_text: accumulated_text,
                    tokens_generated: total_tokens,
                    finish_reason: last_finish_reason.unwrap_or_else(|| {
                        crate::types::FinishReason::Stopped("Unknown".to_string())
                    }),
                    generation_time_ms: 100,
                },
            };

            exchanges.lock().unwrap().push(exchange);
        });

        Ok(Box::pin(ReceiverStream::new(rx)))
    }
}
