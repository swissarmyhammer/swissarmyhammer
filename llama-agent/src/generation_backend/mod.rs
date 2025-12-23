//! Generation backend abstraction for llama-agent
//!
//! This module provides an abstraction layer for generation operations,
//! supporting both real model inference and playback from recorded fixtures.
//!
//! # Purpose
//!
//! Tests that perform real model inference are slow (seconds per test) and require
//! downloading large model files. This module enables recording generation responses
//! to JSON fixtures and playing them back in tests, making tests 100-1000x faster.
//!
//! # Architecture
//!
//! - `GenerationBackend` trait - Abstract interface for generation operations
//! - `RealGenerationBackend` - Uses actual model inference via RequestQueue
//! - `RecordedGenerationBackend` - Plays back from JSON fixtures
//! - `RecordingGenerationBackend` - Records interactions to JSON (optional)
//!
//! # Usage
//!
//! ```rust
//! use llama_agent::generation_backend::{GenerationBackend, RecordedGenerationBackend};
//!
//! // Playback mode for fast tests
//! let backend = RecordedGenerationBackend::from_file("tests/fixtures/test.json")?;
//! let response = backend.generate(&request, &session).await?;
//! ```

use crate::types::{AgentError, GenerationRequest, GenerationResponse, Session, StreamChunk};
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

pub mod real;
pub mod recorded;
pub mod recording;

pub use real::RealGenerationBackend;
pub use recorded::{RecordedGenerationBackend, RecordedSession};
pub use recording::RecordingGenerationBackend;

/// Trait for generation backends
///
/// This trait abstracts generation operations, allowing for:
/// - Real model inference (`RealGenerationBackend`)
/// - Playback from recorded fixtures (`RecordedGenerationBackend`)
/// - Recording interactions (`RecordingGenerationBackend`)
#[async_trait]
pub trait GenerationBackend: Send + Sync {
    /// Generate a complete response (non-streaming)
    async fn generate(
        &self,
        request: &GenerationRequest,
        session: &Session,
    ) -> Result<GenerationResponse, AgentError>;

    /// Generate a streaming response
    async fn generate_stream(
        &self,
        request: &GenerationRequest,
        session: &Session,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, AgentError>> + Send>>, AgentError>;
}
