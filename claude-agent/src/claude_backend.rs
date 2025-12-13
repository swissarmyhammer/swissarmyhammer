//! Backend trait for Claude process interaction
//!
//! This module provides an abstraction layer for communicating with Claude,
//! supporting both real process execution and playback from recorded fixtures.
//!
//! # Purpose
//!
//! Tests that spawn real Claude binaries are slow (~10s each) and can leak processes.
//! This module enables recording Claude I/O to JSON fixtures and playing them back
//! in tests, making tests 100-1000x faster with zero process spawning.
//!
//! # Quick Start
//!
//! **Create a fixture:**
//! ```json
//! {
//!   "exchanges": [
//!     {
//!       "input": "{\"type\":\"user\",\"message\":{...}}",
//!       "outputs": [
//!         "{\"type\":\"assistant\",\"message\":{...}}",
//!         "{\"type\":\"result\",\"status\":\"success\"}"
//!       ]
//!     }
//!   ]
//! }
//! ```
//!
//! **Use in tests:**
//! ```rust
//! let mut backend = RecordedClaudeBackend::from_file("fixtures/my_test.json").unwrap();
//! backend.write_line(INPUT).await.unwrap();
//! let output = backend.read_line().await.unwrap().unwrap();
//! assert!(output.contains("expected text"));
//! ```
//!
//! See `tests/test_prompt_recorded.rs` for complete examples.

use crate::error::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Trait for Claude process backends
///
/// This trait abstracts the I/O operations with Claude, allowing for:
/// - Real process execution (`RealClaudeBackend`)
/// - Playback from recorded fixtures (`RecordedClaudeBackend`)
#[async_trait]
pub trait ClaudeBackend: Send {
    /// Write a line to Claude
    async fn write_line(&mut self, line: &str) -> Result<()>;

    /// Read a line from Claude
    ///
    /// Returns `None` when the stream is exhausted
    async fn read_line(&mut self) -> Result<Option<String>>;

    /// Shutdown the backend
    async fn shutdown(&mut self) -> Result<()>;
}

/// A single exchange in a recorded Claude session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeExchange {
    /// The input sent to Claude (stream-json format)
    pub input: String,
    /// The outputs received from Claude (one per line)
    pub outputs: Vec<String>,
}

/// Recorded Claude session fixture
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedSession {
    /// Sequential exchanges (input -> outputs)
    pub exchanges: Vec<ClaudeExchange>,
}

/// Backend that plays back from a recorded fixture
///
/// This implementation reads from a pre-recorded session fixture,
/// allowing tests to run without spawning real Claude processes.
pub struct RecordedClaudeBackend {
    /// The recorded session data
    session: RecordedSession,
    /// Current exchange index
    exchange_idx: usize,
    /// Output queue for the current exchange
    output_queue: VecDeque<String>,
}

impl RecordedClaudeBackend {
    /// Create a new recorded backend from a fixture
    pub fn new(session: RecordedSession) -> Self {
        Self {
            session,
            exchange_idx: 0,
            output_queue: VecDeque::new(),
        }
    }

    /// Load a recorded session from a JSON file
    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| crate::AgentError::Internal(format!("Failed to read fixture: {}", e)))?;
        let session: RecordedSession = serde_json::from_str(&content).map_err(|e| {
            crate::AgentError::Internal(format!("Failed to parse fixture JSON: {}", e))
        })?;
        Ok(Self::new(session))
    }
}

#[async_trait]
impl ClaudeBackend for RecordedClaudeBackend {
    async fn write_line(&mut self, _line: &str) -> Result<()> {
        // Verify we haven't exhausted the recording
        if self.exchange_idx >= self.session.exchanges.len() {
            return Err(crate::AgentError::Internal(format!(
                "Recorded session exhausted: attempted to write line {} but only {} exchanges recorded",
                self.exchange_idx + 1,
                self.session.exchanges.len()
            )));
        }

        let exchange = &self.session.exchanges[self.exchange_idx];

        // In strict mode, we could verify the input matches the recording
        // For now, we just advance to the next exchange and queue outputs
        tracing::debug!(
            "RecordedBackend: write_line at exchange {}/{}",
            self.exchange_idx + 1,
            self.session.exchanges.len()
        );

        // Queue all outputs for this exchange
        self.output_queue.extend(exchange.outputs.iter().cloned());

        // Advance to next exchange immediately after queueing outputs
        self.exchange_idx += 1;

        Ok(())
    }

    async fn read_line(&mut self) -> Result<Option<String>> {
        // Return queued outputs
        if let Some(output) = self.output_queue.pop_front() {
            tracing::debug!(
                "RecordedBackend: read_line returning queued output ({} bytes)",
                output.len()
            );
            return Ok(Some(output));
        }

        // No more outputs available in queue
        Ok(None)
    }

    async fn shutdown(&mut self) -> Result<()> {
        // No cleanup needed for recorded backend
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_recorded_backend_basic_playback() {
        let session = RecordedSession {
            exchanges: vec![ClaudeExchange {
                input: r#"{"type":"user","message":{"role":"user","content":"Hello"}}"#.to_string(),
                outputs: vec![
                    r#"{"type":"assistant","content":"Hi there!"}"#.to_string(),
                    r#"{"type":"result","status":"success"}"#.to_string(),
                ],
            }],
        };

        let mut backend = RecordedClaudeBackend::new(session);

        // Write input
        backend
            .write_line(r#"{"type":"user","message":{"role":"user","content":"Hello"}}"#)
            .await
            .unwrap();

        // Read outputs
        let line1 = backend.read_line().await.unwrap();
        assert_eq!(
            line1,
            Some(r#"{"type":"assistant","content":"Hi there!"}"#.to_string())
        );

        let line2 = backend.read_line().await.unwrap();
        assert_eq!(
            line2,
            Some(r#"{"type":"result","status":"success"}"#.to_string())
        );

        let line3 = backend.read_line().await.unwrap();
        assert_eq!(line3, None);
    }

    #[tokio::test]
    async fn test_recorded_backend_multiple_exchanges() {
        let session = RecordedSession {
            exchanges: vec![
                ClaudeExchange {
                    input: "input1".to_string(),
                    outputs: vec!["output1a".to_string(), "output1b".to_string()],
                },
                ClaudeExchange {
                    input: "input2".to_string(),
                    outputs: vec!["output2a".to_string()],
                },
            ],
        };

        let mut backend = RecordedClaudeBackend::new(session);

        // First exchange - write queues outputs
        backend.write_line("input1").await.unwrap();
        assert_eq!(
            backend.read_line().await.unwrap(),
            Some("output1a".to_string())
        );
        assert_eq!(
            backend.read_line().await.unwrap(),
            Some("output1b".to_string())
        );
        // After all outputs consumed, returns None
        assert_eq!(backend.read_line().await.unwrap(), None);

        // Second exchange
        backend.write_line("input2").await.unwrap();
        assert_eq!(
            backend.read_line().await.unwrap(),
            Some("output2a".to_string())
        );
        assert_eq!(backend.read_line().await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_recorded_backend_exhausted_session() {
        let session = RecordedSession {
            exchanges: vec![ClaudeExchange {
                input: "input1".to_string(),
                outputs: vec!["output1".to_string()],
            }],
        };

        let mut backend = RecordedClaudeBackend::new(session);

        // Use the one exchange
        backend.write_line("input1").await.unwrap();
        backend.read_line().await.unwrap();

        // Try to write beyond the recording
        let result = backend.write_line("input2").await;
        assert!(result.is_err());
    }
}
